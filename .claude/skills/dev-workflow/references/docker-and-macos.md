# Docker and macOS

## Docker
- `docker/Dockerfile` builds the dev image (Rust toolchain, `just`, `cargo-deny`, `hadolint`, `gitleaks`, `lefthook`); linted by hadolint (`.hadolint.yaml`).
- `compose.yaml`: service `dev`, image `deeboo-lighting-dev`, repo mounted at `/work`, named volumes for caches and `target/`. `.devcontainer/devcontainer.json` opens the repo in the same image.
- Toolchain pinned in `rust-toolchain.toml`; change it and the Dockerfile together. Rebuild after Dockerfile changes: `docker compose build dev`.
- `cargo-deny` config is `deny.toml` (workspace path dependencies allowed; the project is GPL-3.0).

## macOS host
- Needs the Rust toolchain, Node and `just`, plus `lefthook install` (the hook itself runs `just precommit` in Docker).
- Run `just dev` from the host, never inside the container (Docker on a Mac is Linux: no CoreMIDI, audio or bundles).
- Manual acceptance tests: spec §11. 
## CI
- `.github/workflows/ci.yml`: `linux` job runs `just precommit` in the same Docker image; `macos` job (PRs, and pushes touching `src-tauri/`, `tools/`, `ui/`, lockfile, toolchain or workflows) runs clippy and tests with `--locked` and builds the unsigned DMG via `.github/actions/macos-build`. `just ship` pushes and waits for the run of that commit.
- Dependabot keeps cargo, npm, GitHub Actions and the pinned Docker base image (`FROM ...@sha256:`) fresh.
