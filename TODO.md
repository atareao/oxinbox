# TODO — oxinbox

## Estado actual

| Fase | Estado |
|---|---|
| Fase 0: PocketId | ✅ Completado |
| Fase 1: Backend OIDC | ✅ Completado |
| Fase 2: Frontend React (core) | ✅ Completado |
| Fase 3: Frontend React (avanzado) | ◐ Parcial |
| Infraestructura | ◐ Parcial |
| Testing | ◐ Parcial |
| Documentación | ✅ Completado |

---

## Fase 0 — PocketId ✅

- [x] `quadlets/oxinox-auth.container` (PocketId en puerto 8765)
- [x] `quadlets/oxinox.container` (OIDC vars, dependencias)
- [x] `compose.yml` (servicio auth, OIDC env vars)
- [x] `.env.example` (OIDC vars, eliminar WebAuthn)
- [x] `scripts/setup-auth.sh` (admin + cliente OIDC)

## Fase 1 — Backend OIDC ✅

- [x] `Cargo.toml` (webauthn-rs → jsonwebtoken + base64)
- [x] `auth.rs` (OidcConfig, JwtValidator, AuthUser con String)
- [x] `middleware.rs` (JWT Bearer validation)
- [x] `routes/auth.rs` (login redirect, callback, dev-login, /api/me)
- [x] `routes/mod.rs` (nuevas rutas OIDC)
- [x] `main.rs` (sin WebAuthn, init OIDC)
- [x] `database.rs` (user_id: i32 → String, eliminar sessions/users)
- [x] `push.rs`, `geo.rs`, `repository/mod.rs` (user_id String)
- [x] `routes/*.rs` (String user_id, nuevos endpoints)
- [x] `migrations/02_oidc.sql` (ALTER tasks.user_id, DROP sessions/users)
- [x] e2e tests actualizados (9/9 pasando)
- [x] `cargo clippy -- -D warnings` clean

## Fase 2 — Frontend React (core) ✅

- [x] Scaffolding: Vite + React + TypeScript + Ant Design 5
- [x] Auth store (sessionStorage + localStorage)
- [x] API client (fetch wrapper con todos los endpoints)
- [x] React Router v7 con ProtectedRoute
- [x] AppLayout (sidebar con navegación + PushSubscribe + logout)
- [x] LoginPage (PocketId + dev-login)
- [x] HomePage (lista tareas + voz + formulario + búsqueda)
- [x] KanbanPage (@dnd-kit drag & drop)
- [x] CalendarPage (tareas agrupadas por due_date)
- [x] ChatPage (text-to-SQL)
- [x] TaskDetailPage (detalle de tarea con tags)
- [x] VoiceInput (MediaRecorder → WebSocket)
- [x] TaskForm (crear + parse con IA)
- [x] TaskList (check/tags/delete)
- [x] SearchBar (búsqueda híbrida con debounce 300ms)
- [x] StartupReview (inbox estancado >24h)
- [x] Docker multi-stage (pnpm + rust)

## Fase 3 — Frontend React (avanzado) ◐

### PWA
- [x] `manifest.json` con iconos y theme_color
- [x] Code Splitting: `React.lazy()` + `Suspense` para 6 páginas
- [x] `manualChunks`: vendor-react, vendor-antd, vendor-dnd
- [x] ~~Service Worker~~ → **descartado**. Sin offline no tiene sentido. PWA standalone sin SW.

### Push Notifications ✅
- [x] `PushSubscribe.tsx` (toggle en sidebar)
- [x] `GET /api/push/vapid-key` (deriva P-256 public key)
- [x] `p256` crate + `PushService::public_key()`

### Voice WebSocket ✅
- [x] WS streaming cada 250ms a `/api/voice?token=<jwt>`
- [x] Transcripción + parseo con preview
- [x] Creación automática de tarea

### UI Polish ✅
- [x] `ErrorBoundary` global con botón de recarga
- [x] Atajo `Escape` para detener grabación de voz
- [x] Layout responsive (sidebar colapsable en mobile)
- [x] Loading states (Spin en Suspense y ProtectedRoute)
- [x] Loading skeletons (TaskList, KanbanBoard)
- [x] Animaciones: fade-in páginas, pulse grabación, stagger lista tareas, fade-in columnas kanban

## Refactor (2026-07-11) ✅

- [x] Módulo compartido `ai/task_builder.rs` (unifica prompt + creación + LLM parse)
- [x] `routes/ai.rs`, `routes/voice.rs`, `routes/tasks.rs` actualizados
- [x] Auto-assign fallback para text-capture y voice
- [x] `cargo clippy -- -D warnings` clean

## Infraestructura / DevOps ✅

- [x] `podman build` funcional con Dockerfile multi-stage
- [x] `.github/workflows/release.yml` (build binarios + Docker + publish crates.io)
- [x] Healthcheck en quadlets (oxinbox + oxinbox-auth)
- [x] Healthcheck en compose.yml (db + auth + backend)
- [x] Script de backup: `scripts/backup.sh` (PocketId data + PostgreSQL dump, 30-day retention)
- [x] ~~SSL/TLS~~ → **descartado**. El servicio va detrás de reverse proxy existente o solo en LAN

## Testing ◐

- [x] Vitest + Testing Library + jsdom configurado
- [x] `test-setup.ts` con mock de `matchMedia` para Ant Design
- [ ] Tests e2e con OIDC mock (más adelante)
- [ ] Tests unitarios de frontend (TaskList, KanbanBoard) (más adelante)
- [ ] Verificar flujo completo: login → callback → listar → crear → kanban → chat (más adelante)

## Documentación ✅

- [x] `ESPECIFICACIONES.md` actualizado (React, OIDC, Dexie)
- [x] `README.md` actualizado (stack, setup, features)
- [x] TODO.md (este archivo)

---

## Notas

- App: `localhost:3300` (Vite dev: `:5173` → proxy a backend `:3300`)
- PocketId: `localhost:8765`
- Auth: `/auth/login` → PocketId → `/auth/callback?code=...` → JWT en sessionStorage
- Dev: `/auth/dev-login?email=...` → mock JWT
- Todas las rutas `/api/*` requieren `Authorization: Bearer <token>`
- Build: `podman build -t oxinbox:latest .`
- **Sin offline**: requiere conexión continua para LLM+Whisper+ParadeDB. No tiene sentido PWA offline.
