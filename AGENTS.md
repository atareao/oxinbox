# oxinbox — Agent Guide

## Project state

- **Active development.** Single `backend/` crate with data types in `core_types` module, plus `frontend/` (JS/TS). The architecture is specified in `ESPECIFICACIONES.md` (Spanish).
- **Rust edition 2024.** Ensure `cargo` and `rustc` support this; pin a toolchain file if CI complains.

## Specification → code

`ESPECIFICACIONES.md` is the source of truth.

| Crate | Role | Tech |
|---|---|---|
| `oxinbox` (backend/) | HTTP/WS API server + shared data types | Axum, ParadeDB (PostgreSQL + pgvector + BM25), WebAuthn, Whisper/LLM |
| `frontend/` | PWA React client | React 19, Vite, antd, Dexie.js, Service Worker |

- Task IDs are **UUID v7** (time-sortable). Enabled via `uuid/v7` feature.

## Commands

```sh
cargo build --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml
cargo fmt --check
```

### Pre-commit checklist

**Siempre** pasar clippy antes de commitear en cualquier rama:

```sh
cargo clippy --manifest-path backend/Cargo.toml -- -D warnings
```

No commitees si clippy falla. Esto asegura que `release.yml` nunca falle por lint warnings en código existente.

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
