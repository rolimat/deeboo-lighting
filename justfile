set shell := ["bash", "-euo", "pipefail", "-c"]

# Recipes that need the toolchain re-run themselves inside Docker on the host.
docker := "docker compose run --rm dev just"

default:
    @just --list

fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} fmt
    if [ -f ui/package.json ]; then just ui-deps; fi
    cargo fmt --all
    if [ -f ui/package.json ]; then npm --prefix ui run format; fi

fmt-check:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} fmt-check
    if [ -f ui/package.json ]; then just ui-deps; fi
    cargo fmt --all --check
    if [ -f ui/package.json ]; then npm --prefix ui run format:check; fi

lint:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} lint
    if [ -f ui/package.json ]; then just ui-dist; fi
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo deny check
    hadolint docker/Dockerfile
    scripts/check-agent-docs.sh
    gitleaks git --no-banner --redact
    if [ -f ui/package.json ]; then npm --prefix ui run lint && npm --prefix ui run typecheck && npm --prefix ui audit --audit-level=high; fi

test:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} test
    if [ -f ui/package.json ]; then just ui-dist; fi
    cargo test --locked --workspace
    if [ -f ui/package.json ]; then npm --prefix ui test -- --run; fi

# Performance budget (spec 13.1): 10 min of audio analysed in < 3 s, release build.
perf-core:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} perf-core
    cargo test --locked --release -p light-core --test perf -- --ignored --nocapture

# Builds ui/dist: tauri::generate_context! needs it before cargo compiles src-tauri.
ui-dist: ui-deps
    npm --prefix ui run build

# Installs ui/node_modules when empty (the Docker volume starts empty).
ui-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "$(ls -A ui/node_modules 2>/dev/null)" ]; then npm --prefix ui ci; fi

precommit:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} precommit
    gitleaks git --pre-commit --staged --no-banner --redact
    just fmt-check
    just lint
    just test

check: precommit

dev:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just dev runs on the macOS host only" >&2; exit 1; }
    export PATH="$HOME/.cargo/bin:$PATH"
    if [ ! -f ui/node_modules/.bin/tauri ]; then npm --prefix ui ci; fi
    npm --prefix ui run tauri dev

# Smart Life QR login on the macOS host: lists devices, saves the strip to .dev/strip.json (0600).
tuya-login *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just tuya-login runs on the macOS host only" >&2; exit 1; }
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo run --release -p tuya-login -- {{ARGS}}

# Push, wait for the GitHub Actions run of this exact commit, fail if it fails.
ship:
    #!/usr/bin/env bash
    set -euo pipefail
    git push -u origin HEAD
    sha=$(git rev-parse HEAD)
    id=""
    for _ in $(seq 1 30); do
        id=$(gh run list --commit "$sha" --json databaseId -q '.[0].databaseId // empty')
        [ -n "$id" ] && break
        sleep 2
    done
    [ -n "$id" ] || { echo "no CI run found for $sha" >&2; exit 1; }
    gh run watch --exit-status "$id"
