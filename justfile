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
    # gitleaks scans nothing (and passes) when git cannot read the repo: fail closed instead.
    git rev-parse --is-inside-work-tree >/dev/null
    gitleaks git --no-banner --redact
    if [ -f ui/package.json ]; then npm --prefix ui run lint && npm --prefix ui run typecheck && npm --prefix ui audit --audit-level=high; fi

test:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} test
    if [ -f ui/package.json ]; then just ui-dist; fi
    cargo test --locked --workspace
    scripts/tests/log-scan.sh
    if [ -f ui/package.json ]; then npm --prefix ui test -- --run; fi

# Performance budget (spec 13.1): 10 min of audio analysed in < 3 s, release build.
perf-core:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} perf-core
    cargo test --locked --release -p light-core --test perf -- --ignored --nocapture

# macOS: samples CPU/RSS of the running app (launches the release bundle if needed) against the 13.1 targets; reports only.
perf SECONDS="60":
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just perf runs on the macOS host only" >&2; exit 1; }
    scripts/perf.sh {{SECONDS}}

# Fails (exit 1, line numbers only) if a session log holds a secret shape.
log-scan FILE KEY="":
    scripts/log-scan.sh {{FILE}} {{KEY}}

# Frees build caches (host target/, the Docker target volume, Docker build cache); all rebuildable.
clean-cache:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo clean
    if docker info >/dev/null 2>&1; then
        docker volume rm -f "$(basename "$PWD")_target" >/dev/null
        docker builder prune -f >/dev/null
    fi
    df -h ~ | tail -1

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
    git rev-parse --is-inside-work-tree >/dev/null
    gitleaks git --pre-commit --staged --no-banner --redact
    just fmt-check
    just lint
    just test

check: precommit

# Release DMG (ad-hoc signed) in target/release/bundle/dmg, macOS host only.
dmg:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just dmg runs on the macOS host only" >&2; exit 1; }
    export PATH="$HOME/.cargo/bin:$PATH"
    if [ ! -f ui/node_modules/.bin/tauri ]; then npm --prefix ui ci; fi
    npm --prefix ui run tauri build -- --bundles dmg

dev:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just dev runs on the macOS host only" >&2; exit 1; }
    export PATH="$HOME/.cargo/bin:$PATH"
    if [ ! -f ui/node_modules/.bin/tauri ]; then npm --prefix ui ci; fi
    npm --prefix ui run tauri dev

# Smart Life QR login on the macOS host: lists devices, saves the strip to .dev/strip.json (0600).
[positional-arguments]
tuya-login *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just tuya-login runs on the macOS host only" >&2; exit 1; }
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo run --release -p tuya-login -- "$@"

# macOS: installs .dev/strip.json (mode 0600) where the sandboxed app reads its strip file (`app_config_dir`).
install-strip:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just install-strip runs on the macOS host only" >&2; exit 1; }
    [ -f .dev/strip.json ] || { echo ".dev/strip.json is missing: run just tuya-login first" >&2; exit 1; }
    dest="$HOME/Library/Containers/com.deeboo.lighting/Data/Library/Application Support/com.deeboo.lighting"
    mkdir -p "$dest"
    install -m 0600 .dev/strip.json "$dest/strip.json"
    echo "installed $dest/strip.json"

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

# Offline detector run on a recording (macOS host, release build): episodes, --timeline, --labels, --bpm, --realtime.
[positional-arguments]
detect-probe *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname)" = Darwin ] || { echo "just detect-probe runs on the macOS host only" >&2; exit 1; }
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo run --release -p detect-probe -- "$@"

# Regenerates the committed menu bar frames in src-tauri/icons/tray (runs in Docker; output is deterministic).
tray-frames:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${DEEBOO_IN_CONTAINER:-}" ] || exec {{docker}} tray-frames
    cargo run --locked -p tray-frames
