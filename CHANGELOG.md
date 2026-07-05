# Changelog
## [0.1.3] - 2026-07-05

### Bug Fixes

- Remove --locked flag from release builds
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
