# oxinbox

![Rust](https://img.shields.io/badge/edition-2024-dea584?logo=rust&style=flat-square)
![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
![ParadeDB](https://img.shields.io/badge/ParadeDB-PostgreSQL%2Bpgvector-4169E1?style=flat-square&logo=postgresql)
![OIDC](https://img.shields.io/badge/auth-OIDC%20(PocketId)-0052CC?style=flat-square)

**Productividad GTD invisible y asistida por IA, con backend en Rust.**

oxinbox es una aplicación web progresiva (PWA) de fricción cero para la metodología GTD. Captura tareas por voz o texto, las estructura con un LLM, y permite consultas conversacionales sobre tu historial y planificación.

---

## Stack

| Capa | Tecnología |
|---|---|
| Cliente | **React** + Vite + Ant Design 5, IndexedDB (Dexie.js), Service Worker |
| Servidor | **Axum** (Rust), **ParadeDB** (PostgreSQL + pgvector + BM25) |
| Auth | **OIDC** via **PocketId** (self-hosted, passkeys) |
| IA | Whisper (STT), LLM (estructuración + Text-to-SQL), embeddings vía **OpenRouter** |
| Infra | Podman Quadlets, Docker Compose, multi-stage build |

## Arquitectura

```
oxinbox/
├── core/          # Tipos compartidos (Task, TaskStatus, UUID v7)
├── backend/       # API HTTP/WS con Axum + ParadeDB
└── frontend/      # PWA React con sincronización offline
```

## Empezar

```bash
# Clonar
git clone https://github.com/<tu-org>/oxinbox
cd oxinbox

# Iniciar servicios
docker compose up -d

# Configurar auth (PocketId admin + OIDC client por primera vez)
./scripts/setup-auth.sh

# Variables de entorno
cp .env.example .env
# Editar .env con API key de OpenRouter (https://openrouter.ai/keys)
# y OIDC_CLIENT_SECRET (generado por setup-auth.sh)
```

**Nota**: El frontend React se construye automáticamente en el Docker multi-stage. Para desarrollo local:

```bash
cd frontend
pnpm install
pnpm run dev    # http://localhost:5173 con proxy al backend :3300
```

## Características principales

- **Captura por voz**: micrófono → WebSocket → transcripción Whisper → estructuración LLM → persistencia
- **Búsqueda híbrida**: BM25 + similitud coseno combinados con RRF ponderado
- **Offline-first**: escrituras optimistas sobre IndexedDB (Dexie.js), sincronización en lote con last-write-wins
- **Kanban drag & drop**: columnas Inbox/Todo/Doing/Done/Someday con @dnd-kit
- **Automatizaciones GTD**: micro-revisiones pasivas de Inbox, notificaciones push
- **Memoria asistida**: preguntas en lenguaje natural, el LLM traduce a SQL sobre el histórico

## Requisitos

- Rust 1.75+ (toolchain: `nightly-2025-03-01`)
- Node.js 23 + pnpm (para frontend)
- Docker + Docker Compose (para ParadeDB)
- API key de OpenRouter (https://openrouter.ai/keys)
- PocketId admin credentials (generadas por setup-auth.sh)

## Licencia

[MIT](LICENSE)