---
name: dev-workflow
description: Use when setting up the environment, running just commands, working with Docker, lefthook, CI or releases, committing, or deciding what runs on macOS versus in the container.
---

# Dev workflow

Spec §10: [docs/superpowers/specs/2026-11-06-deeboo-lighting-design.md](../../../docs/superpowers/specs/2026-11-06-deeboo-lighting-design.md).

## Where things run
- Docker (image `deeboo-lighting-dev`): fmt, lint, tests. Recipes re-exec through `docker compose run --rm dev just <recipe>` when `DEEBOO_IN_CONTAINER` is unset, so run them from the host as usual.
- macOS host only: `just dev` (needs `ui/` and `src-tauri/`), real MIDI, audio, the strip and the DMG.

## Recipes
| Recipe | Does |
|---|---|
| `just fmt` / `fmt-check` | `cargo fmt`; UI Prettier when `ui/package.json` exists |
| `just lint` | clippy `-D warnings`, `cargo deny`, hadolint, gitleaks, `scripts/check-agent-docs.sh`, UI eslint + typecheck |
| `just test` | `cargo test --workspace`; Vitest when `ui/package.json` exists |
| `just precommit` (`check`) | gitleaks on staged changes, then fmt-check + lint + test |
| `just perf-core` | release-mode perf budget test (10 min of audio in < 3 s); CI Linux job runs it |
| `just perf [SECONDS=60]` | macOS only: samples CPU/RSS of the `deeboo-lighting` process (launches the release bundle if not running) and prints mean/max beside the 13.1 targets; reports, never fails |
| `just log-scan <file>` | exit 1 and line numbers (never the text) if a log holds a secret shape; fixtures in `scripts/tests/log-scan`, run by `just test` |
| `just clean-cache` | host only: frees rebuildable caches (`cargo clean`, the Docker `target` volume, Docker build cache); run after each plan merge or when the disk is low |
| `just ui-dist` / `ui-deps` | build `ui/dist` (needed by `tauri::generate_context!`) / install `ui/node_modules` when empty |
| `just dmg` | macOS only: release build, ad-hoc signed DMG in `target/release/bundle/dmg/` |
| `just dev` | macOS only: `npm --prefix ui run tauri dev` |
| `just tuya-login` | macOS only: Smart Life QR login, lists devices, saves the strip to `.dev/strip.json` (0600) |
| `just detect-probe <audio> [--labels f.json] [--timeline] [--bpm N] [--realtime] [--downbeat S]... [--phrase 4\|8\|16] [--rules all\|none\|calm,drop,riser,bars]` | macOS only: runs the detector on an MP3/WAV in the live-app event order (recordings stay in ignored `recordings/`); `--realtime` plays it on the default output; `--downbeat` (seconds, repeatable, snaps to the nearest tracked beat) fixes phrase position, `--rules` switches the position rules |
| `just tray-frames` | Docker: regenerates the committed menu bar PNGs in `src-tauri/icons/tray` (deterministic; commit the output) |
| `just ship` | push and wait for the CI run of this exact commit |

## Rules
- Hook: lefthook pre-commit runs `just precommit`. Never `--no-verify`; fix the cause.
- Conventional Commits (`feat(scope):`, `fix:`, `docs:`, `chore:`, ...).
- Release: pushing a `v*` tag (user's call) runs `.github/workflows/release.yml`: macOS build job (`contents: read`), then publish job (`contents: write`, `id-token`, `attestations`) with `SHA256SUMS`, build provenance and a GitHub release holding the ad-hoc signed DMG.
- Feature branch, never `main`. Ask before pushing any tag.
- Agent docs have a budget enforced by `scripts/check-agent-docs.sh`: `AGENTS.md` <= 80 lines, each `SKILL.md` <= 100 lines, description <= 250 chars, every `references/*.md` linked from its `SKILL.md`. `rust-engineer` is exempt (vendored unchanged).
- Convention, command or interface change: update the matching skill in the same commit.

## Details
- Image, compose, volumes, macOS steps: [references/docker-and-macos.md](references/docker-and-macos.md) (read when env or Docker issues arise).

## Checklist
- [ ] `just precommit` green before commit.
- [ ] Commit message is Conventional.
