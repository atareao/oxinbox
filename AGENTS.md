# oxinbox — Agent Guide

## Project state

- **Active development.** Workspace structure with `core/`, `backend/`, `frontend/` crates. The architecture is specified in `ESPECIFICACIONES.md` (Spanish).
- **Rust edition 2024.** Ensure `cargo` and `rustc` support this; pin a toolchain file if CI complains.

## Specification → code

`ESPECIFICACIONES.md` is the source of truth. The system will become a Cargo workspace:

| Crate | Role | Tech |
|---|---|---|
| `core/` | Shared data types (Task, TaskStatus, TaskHistory, UUID v7) | serde, chrono, uuid |
| `backend/` | HTTP/WS API server | Axum, ParadeDB (PostgreSQL + pgvector + BM25), WebAuthn, Whisper/LLM |
| `frontend/` | PWA WebAssembly client | Dioxus, IndexedDB sync, Service Worker |

- The root `Cargo.toml` must be the **workspace** root (not a package). Migrate the current single crate to `core/` or `backend/`.
- Task IDs are **UUID v7** (time-sortable). Enable `uuid/v7` feature.

## Commands

```sh
cargo build               # build all workspace members
cargo build -p oxinbox-core
cargo build -p oxinbox-backend
cargo test                # run all tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Git Flow with conventional commits + gitmoji. See [GIT_FLOW.md](./GIT_FLOW.md).

## Conventions

- `ESPECIFICACIONES.md` drives all architecture decisions. Read it before implementing any feature.
- All user communication / spec language is Spanish. Code identifiers, comments, and commit messages should match accordingly.
- Database: ParadeDB (PostgreSQL + pgvector). Embeddings are 1024-dimensional `vector(1024)`.
- Auth: WebAuthn (passkeys) via `webauthn-rs`, no passwords. Sessions last 1 year.
- Sync: offline-first with IndexedDB + last-write-wins conflict resolution.
- Search: hybrid BM25 + cosine similarity via Reciprocal Rank Fusion.
- AI: OpenRouter gateway with `deepseek/deepseek-v4-flash` (chat), `baai/bge-m3` (embeddings), `qwen/qwen3-asr-flash-2026-02-10` (transcription).

## Git Flow

This project follows strict gitflow. See [GIT_FLOW.md](./GIT_FLOW.md) for:
- Branch structure (main, development, feature/*, hotfix/*)
- Conventional commits with gitmoji
- How to create features, hotfixes, and releases