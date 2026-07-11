# Changelog
## [0.1.12] - 2026-07-11

### Features

- Modernize oxinbox — React frontend, OIDC auth, projects and contexts
- Modernize oxinbox — React frontend, OIDC auth, projects and contexts

### Other

- V0.2.0 — React frontend, OIDC auth, projects and contexts
## [0.1.11] - 2026-07-05

### Bug Fixes

- Address clippy warnings in frontend_fallback (#15)

### Miscellaneous Tasks

- Release v0.1.11
## [0.1.10] - 2026-07-05

### Bug Fixes

- Build frontend WASM in Dockerfile and CI pipeline (#14)

### Miscellaneous Tasks

- Release v0.1.10
## [0.1.9] - 2026-07-05

### Bug Fixes

- Use C.UTF-8 locale for PostgreSQL in docker-compose files (#8)

### Miscellaneous Tasks

- Release v0.1.9
## [0.1.8] - 2026-07-05

### Bug Fixes

- Fix release sync to development branch (#13)

### Miscellaneous Tasks

- Release v0.1.8
## [0.1.7] - 2026-07-05

### Bug Fixes

- Fix rustfmt formatting in main.rs (#12)

### Miscellaneous Tasks

- Release v0.1.7
## [0.1.6] - 2026-07-05

### Bug Fixes

- Serve frontend static files from backend (#11)

### Miscellaneous Tasks

- Release v0.1.6
## [0.1.5] - 2026-07-05

### Bug Fixes

- Fix rustfmt formatting in auth.rs (#10)

### Miscellaneous Tasks

- Release v0.1.5
## [0.1.4] - 2026-07-05

### Bug Fixes

- Add wget to Docker image for health check (#9)

### Miscellaneous Tasks

- Release v0.1.4
## [0.1.3] - 2026-07-05

### Bug Fixes

- Remove --locked flag from release builds

### Miscellaneous Tasks

- Release v0.1.3
## [0.1.2] - 2026-07-05

### Bug Fixes

- Update Cargo.lock for --locked builds
- Read version from backend/.vampus.yml in release workflow

### Miscellaneous Tasks

- Release v0.1.2
## [0.1.0] - 2026-07-05

### Bug Fixes

- Pass GITHUB_TOKEN to release action
- Dockerfile workspace + release-prepare token + publish resilience
- Add openssl-libs-static and guard release job by tag
- Correct extension names and BM25 syntax in migration
- Fix PostgreSQL migration extensions for ParadeDB pgvector compat
- Fix vampus config to target backend/Cargo.toml
- Fix vampus config to target backend/Cargo.toml
- Add db connection retry and spanish locale/tz config
- Move vampus config to backend/ for workspace compat

### Documentation

- Add CHANGELOG for v0.1.0

### Miscellaneous Tasks

- Initial project setup
- Add crates.io metadata to core and backend
- Add Docker build & push to release workflow
- Add workflow_dispatch to release.yml
- Add rustfmt and clippy components in toolchain setup
- Add rustfmt and clippy to rust-toolchain.toml components
- Release v0.1.0

### Refactor

- Remove GH_PAT dependency, use GITHUB_TOKEN everywhere

### Styling

- Cargo fmt on all workspace files
