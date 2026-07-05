# Especificación Técnica: `oxinbox`

## Herramienta de Productividad GTD Invisible y Asistida por IA

`oxinbox` es una aplicación web progresiva (PWA) de alta velocidad orientada a la metodología GTD (Getting Things Done) y desarrollada íntegramente en **Rust**. Su pilar fundamental es la **fricción cero**: permitir al usuario vaciar su mente de forma instantánea a través de comandos de voz y texto procesados por un Modelo de Lenguaje (LLM), eliminando la necesidad de organizar metadatos manualmente y permitiendo consultas conversacionales avanzadas sobre el historial y planificación del usuario.

---

## 1. Arquitectura del Sistema: Cargo Workspace

Para maximizar la eficiencia y evitar la duplicación de código, el proyecto se organiza como un espacio de trabajo de Cargo mono-repositorio, compartiendo estructuras de datos nativas entre el cliente WebAssembly y el servidor en Rust.

```text
oxinbox/
├── Cargo.toml                  # Raíz del Workspace
├── core/                       # Biblioteca de estructuras y lógica compartida
│   ├── Cargo.toml
│   └── src/lib.rs
├── backend/                    # Servidor API HTTP/WebSockets (Axum + ParadeDB)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── auth.rs             # WebAuthn / Passkeys
│       ├── database.rs         # Pool de conexiones y consultas ParadeDB
│       ├── ai.rs               # Integración con Whisper, LLM y Text-to-SQL
│       └── push.rs             # Motor de notificaciones web push
└── frontend/                   # Interfaz PWA en WebAssembly (Dioxus)
    ├── Cargo.toml
    ├── Dioxus.toml
    └── src/
        ├── main.rs
        ├── storage.rs          # Sincronización local con IndexedDB
        └── components/         # Chat de voz, Kanban, Calendario

```

### `Cargo.toml` (Raíz del Workspace)

```toml
[workspace]
resolver = "2"
members = [
    "core",
    "backend",
    "frontend"
]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.10", features = ["v7", "serde"] }

```

---

## 2. Modelo de Datos y Trazabilidad (`core`)

El núcleo conceptual de `oxinbox` utiliza la flexibilidad del formato léxico de `todo.txt` pero estructurado de forma relacional, tipada y optimizada para búsquedas vectoriales. La creación de tareas offline requiere identificadores únicos **UUID v7** (secuenciales basados en tiempo) para evitar colisiones lógicas durante la sincronización remota.

### `core/src/lib.rs`

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDate};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Inbox,
    Todo,
    Doing,
    Done,
    Someday,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: Uuid,                  // UUID v7 secuencial por tiempo
    pub completed: bool,
    pub priority: Option<char>,    // 'A'..'Z'
    pub description: String,       // Texto limpio de la tarea
    pub projects: Vec<String>,     // Colección de +proyectos
    pub contexts: Vec<String>,     // Colección de @contextos
    pub status: TaskStatus,
    
    // Trazabilidad temporal estricta
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskHistory {
    pub id: i32,
    pub task_id: Uuid,
    pub from_status: Option<TaskStatus>,
    pub to_status: TaskStatus,
    pub changed_at: DateTime<Utc>,
}

```

---

## 3. Base de Datos Híbrida: ParadeDB

El backend utiliza **ParadeDB** sobre PostgreSQL. Esta base de datos unifica la búsqueda léxica convencional (BM25 de `pg_search`) con la búsqueda semántica vectorial (`pgvector`), permitiendo consultas híbridas avanzadas en una sola infraestructura.

### Esquema de Migración SQL (`backend/migrations/01_init.sql`)

```sql
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS pgvector;
CREATE EXTENSION IF NOT EXISTS paradedb;

-- Tabla principal de usuarios
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Tabla de sesiones persistentes
CREATE TABLE sessions (
    token VARCHAR(64) PRIMARY KEY,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Tabla de tareas indexada con vectores
CREATE TABLE tasks (
    id UUID PRIMARY KEY, -- UUID v7 inyectado desde el cliente o backend
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    completed BOOLEAN DEFAULT FALSE NOT NULL,
    priority CHAR(1) CHECK (priority >= 'A' AND priority <= 'Z'),
    description TEXT NOT NULL,
    projects TEXT[] DEFAULT '{}'::TEXT[] NOT NULL,
    contexts TEXT[] DEFAULT '{}'::TEXT[] NOT NULL,
    status VARCHAR(20) DEFAULT 'inbox' NOT NULL,
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at TIMESTAMP WITH TIME ZONE,
    due_date DATE,
    
    -- Embeddings vectoriales (1024 dimensiones para BGE-M3)
    embedding vector(1024)
);

-- Tabla de historial analítico para auditoría de flujos GTD
CREATE TABLE task_history (
    id SERIAL PRIMARY KEY,
    task_id UUID REFERENCES tasks(id) ON DELETE CASCADE NOT NULL,
    from_status VARCHAR(20),
    to_status VARCHAR(20) NOT NULL,
    changed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Índices Híbridos (ParadeDB BM25 + Vector HNSW)
CREATE INDEX idx_tasks_bm25 ON tasks USING prdb (description, projects, contexts);
CREATE INDEX idx_tasks_embedding ON tasks USING hnsw (embedding vector_cosine_ops);

-- Trigger automático para auditoría de estados y marcas de tiempo
CREATE OR REPLACE FUNCTION process_task_modifications()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    
    -- Registrar cambios en el historial de flujos GTD
    IF (OLD IS NULL OR OLD.status IS DISTINCT FROM NEW.status) THEN
        INSERT INTO task_history (task_id, from_status, to_status)
        VALUES (NEW.id, OLD.status, NEW.status);
    END IF;
    
    -- Gestión de completado implícito
    IF NEW.status = 'done' AND (OLD IS NULL OR OLD.status IS DISTINCT FROM 'done') THEN
        NEW.completed_at = CURRENT_TIMESTAMP;
        NEW.completed = TRUE;
    ELSIF NEW.status IS DISTINCT FROM 'done' THEN
        NEW.completed_at = NULL;
        NEW.completed = FALSE;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_tasks_telemetry
BEFORE INSERT OR UPDATE ON tasks
FOR EACH ROW EXECUTE FUNCTION process_task_modifications();

```

---

## 4. Estrategia de Autenticación Fricción Cero: WebAuthn

Para evitar pantallas de login complejas y contraseñas obsoletas, `oxinbox` implementa el estándar biométrico **WebAuthn (Passkeys)** mediante el ecosistema `webauthn-rs`.

1. **Persistencia:** Una vez validada la firma criptográfica del dispositivo móvil o escritorio, el backend de Axum expide un **Token de Sesión de Larga Duración** (almacenado en la tabla `sessions` con caducidad a 1 año) que se guarda localmente en la PWA.
2. **Validación:** El middleware de Axum intercepta las cabeceras HTTP (`Authorization: Bearer <TOKEN>`) y valida la sesión contra la base de datos en menos de $1\text{ ms}$, exponiendo un struct `AuthUser` seguro a los endpoints protegidos.

---

## 5. El Flujo de Captura y Algoritmo Híbrido

### Flujo de Captura por Voz (Fricción Cero)

El usuario activa el micrófono desde la interfaz táctil o mediante un atajo global de teclado/pantalla de bloqueo.

```
[PWA / Audio Stream] ──(WebSocket)──> [Axum Backend]
                                             │
                                             ▼
                                    [Whisper API / Local]
                                             │
                                             ▼
                                     [Texto Transcrito]
                                             │
                                             ▼
                                    [Estructuración LLM]
                                 (Extrae campos todo.txt)
                                             │
                                             ▼
                                    [Generar Embedding]
                                             │
                                             ▼
                                  [Persistencia en DB]

```

### Algoritmo de Búsqueda Híbrida (Reciprocal Rank Fusion - RRF Ponderado)

Para buscar tareas de manera eficiente combinando coincidencia exacta de palabras clave (`+proyecto`, `@contexto`) y proximidad semántica abstracta, se ejecuta una consulta unificada de ordenación ponderada en ParadeDB:

$$Score = (1 - (\text{embedding} \Leftrightarrow \text{query\_vector})) \times 0.7 + (\text{paradedb.score}(id) \times 0.3)$$

---

## 6. Motor de Consultas Temporales y Lenguaje Natural (Memoria Asistida)

`oxinbox` debe responder como un ser humano a las dudas organizativas cotidianas del usuario utilizando capacidades de **Text-to-SQL / Tool Calling** del LLM acoplado. El sistema traduce intenciones abstractas en consultas exactas sobre la tabla `tasks` y `task_history`.

El LLM recibe en su *System Prompt* la fecha y hora actuales del sistema (ej: `2026-07-04`) y el esquema relacional para ejecutar flujos de respuesta guiados:

### A. Intención: ¿Cuándo hice esta tarea?

* **Consulta Humana:** *"¿Cuándo completé la tarea de configurar el proxy inverso?"*
* **Resolución técnica:**
1. El LLM localiza el `id` de la tarea mediante embedding o búsqueda semántica.
2. Ejecuta una consulta sobre la telemetría histórica:


```sql
SELECT changed_at FROM task_history 
WHERE task_id = $1 AND to_status = 'done' 
ORDER BY changed_at DESC LIMIT 1;

```


* **Respuesta generada:** *"Completaste esa tarea ayer viernes por la tarde a las 18:30h."*

### B. Intención: ¿Cuándo tengo que hacer algo?

* **Consulta Humana:** *"¿Para cuándo tengo programado el mantenimiento del servidor?"*
* **Resolución técnica:**
```sql
SELECT due_date, status FROM tasks 
WHERE description ILIKE '%mantenimiento%servidor%' AND completed = FALSE 
LIMIT 1;

```


* **Respuesta generada:** *"El mantenimiento está en tu columna 'Todo' y tiene como fecha límite este próximo martes, 7 de julio."*

### C. Intención: Auditoría retrospectiva de actividad

* **Consulta Humana:** *"¿Qué estuve haciendo ayer por la tarde?"*
* **Resolución técnica:**
```sql
SELECT t.description, h.to_status, h.changed_at 
FROM task_history h
JOIN tasks t ON h.task_id = t.id
WHERE h.changed_at BETWEEN '2026-07-03 15:00:00+02' AND '2026-07-03 21:00:00+02'
ORDER BY h.changed_at ASC;

```


* **Respuesta generada:** *"Ayer por la tarde avanzaste a buen ritmo en +Voltio: moviste tres tareas a 'Doing' y cerraste la integración con SQLite."*

---

## 7. Sincronización Offline y Ciclo de Vida PWA

La interfaz construida con **Dioxus (WebAssembly)** se comporta de forma totalmente autónoma cuando no detecta conectividad de red:

* **Escrituras Offline (Optimistas):** Las inserciones y transiciones del Kanban modifican inmediatamente el estado reactivo local respaldado por **IndexedDB**, garantizando tiempos de respuesta de $0\text{ ms}$ en la interfaz.
* **Cola de Sincronización en Lote:** Al restablecerse la conexión de red (evento `online`), un Service Worker en segundo plano procesa las mutaciones encoladas enviándolas en lote (*batch transaction*) al backend.
* **Resolución de Conflictos:** Se aplica la política *Last-Write-Wins* evaluando la marca temporal estricta de `updated_at`. La base de datos mantiene las versiones anteriores en `task_history` permitiendo reversiones lógicas si el usuario lo solicita a la IA.

---

## 8. Automatizaciones GTD Proactivas

* **Micro-Revisiones de Flujo Pasivas:** Tareas en segundo plano en Axum analizan periódicamente el estado de ParadeDB mediante hilos de ejecución `tokio`. Si detectan tareas estancadas en `Inbox` por más de 24 horas, el LLM sintetiza un breve resumen de voz interactivo al iniciar la aplicación: *"Tienes 3 notas en el Inbox desde ayer, ¿las clasificamos ahora en 10 segundos?"*.
* **Notificaciones de Contexto por Geolocalización:** La PWA registra cambios de coordenadas espaciales nativas de forma pasiva y eficiente. Si el usuario se aproxima a una ubicación vinculada semánticamente a tareas en el contexto `@compra` u `@oficina`, el backend despacha una notificación Web Push instantánea con los pendientes específicos.
