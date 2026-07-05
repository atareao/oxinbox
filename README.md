# oxinbox

![Rust](https://img.shields.io/badge/edition-2024-dea584?logo=rust&style=flat-square)
![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)
![ParadeDB](https://img.shields.io/badge/ParadeDB-PostgreSQL%2Bpgvector-4169E1?style=flat-square&logo=postgresql)
![WebAuthn](https://img.shields.io/badge/auth-WebAuthn%20passkeys-0052CC?style=flat-square)

**Productividad GTD invisible y asistida por IA, 100 % en Rust.**

oxinbox es una aplicación web progresiva (PWA) de fricción cero para la metodología GTD. Captura tareas por voz o texto, las estructura con un LLM, y permite consultas conversacionales sobre tu historial y planificación — sin pantallas de login, sin etiquetado manual, sin excusas.

---

## Stack

| Capa | Tecnología |
|---|---|
| Cliente | **Dioxus** (WebAssembly), IndexedDB, Service Worker |
| Servidor | **Axum**, **ParadeDB** (PostgreSQL + pgvector + BM25) |
| Auth | **WebAuthn** (passkeys), sesiones de 1 año |
| IA | Whisper (STT), LLM (estructuración + Text-to-SQL), embeddings 1536d vía **OpenRouter** |
| Infra | Docker Compose, multi-stage build |

## Arquitectura

```
oxinbox/
├── core/          # Tipos compartidos (Task, TaskStatus, UUID v7)
├── backend/       # API HTTP/WS con Axum + ParadeDB
└── frontend/      # PWA Dioxus con sincronización offline
```

## Empezar

```bash
# Clonar
git clone https://github.com/<tu-org>/oxinbox
cd oxinbox

# Iniciar base de datos y servidor
docker compose up -d

# Construir backend
cargo build -p oxinbox-backend

# Variables de entorno
cp .env.example .env
# Editar .env con tu API key de OpenRouter (https://openrouter.ai/keys)
```

## Características principales

- **Captura por voz**: micrófono → transcripción Whisper → estructuración LLM → persistencia
- **Búsqueda híbrida**: BM25 + similitud coseno combinados con RRF ponderado
- **Offline-first**: escrituras optimistas sobre IndexedDB, sincronización en lote con last-write-wins
- **Automatizaciones GTD**: micro-revisiones pasivas de Inbox, notificaciones por geolocalización
- **Memoria asistida**: preguntas en lenguaje natural, el LLM traduce a SQL sobre el histórico

## Requisitos

- Rust 1.75+ (toolchain: `1.95.0`)
- Docker + Docker Compose (para ParadeDB)
- API key de OpenRouter (https://openrouter.ai/keys)

## Licencia

[MIT](LICENSE)