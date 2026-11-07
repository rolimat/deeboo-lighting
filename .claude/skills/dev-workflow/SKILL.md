---
name: dev-workflow
description: Use when setting up the environment, running just commands, working with Docker, lefthook, CI or releases, committing, or deciding what runs on macOS versus in the container.
---

# Dev workflow

Spec §10: [docs/superpowers/specs/2026-11-06-deeboo-lighting-design.md](../../../docs/superpowers/specs/2026-11-06-deeboo-lighting-design.md).

## Where things run
- Docker (image `deeboo-dev`): fmt, lint, tests. Recipes re-exec through `docker compose run --rm dev just <recipe>` when `DEEBOO_IN_CONTAINER` is unset, so run them from the host as usual.
- macOS host only: `just dev` (needs `ui/` and `src-tauri/`), real MIDI, audio, the strip and the DMG.

## Recipes
| Recipe | Does |
|---|---|
| `just fmt` / `fmt-check` | `cargo fmt`; UI Prettier when `ui/package.json` exists |
| `just lint` | clippy `-D warnings`, `cargo deny`, hadolint, gitleaks, `scripts/check-agent-docs.sh`, UI eslint + typecheck |
| `just test` | `cargo test --workspace`; Vitest when `ui/package.json` exists |
| `just precommit` (`check`) | fmt-check + lint + test |
| `just ui-dist` / `ui-deps` | build `ui/dist` (needed by `tauri::generate_context!`) / install `ui/node_modules` when empty |
| `just dev` | macOS only: `npm --prefix ui run tauri dev` |
| `just ship` | push and wait for the CI run of this exact commit |

## Rules
- Hook: lefthook pre-commit runs `just precommit`. Never `--no-verify`; fix the cause.
- Conventional Commits (`feat(scope):`, `fix:`, `docs:`, `chore:`, ...).
- Feature branch, never `main`. Ask before pushing any tag.
- Agent docs have a budget enforced by `scripts/check-agent-docs.sh`: `AGENTS.md` <= 80 lines, each `SKILL.md` <= 100 lines, description <= 250 chars, every `references/*.md` linked from its `SKILL.md`. `rust-engineer` is exempt (vendored unchanged).
- Convention, command or interface change: update the matching skill in the same commit.

## Details
- Image, compose, volumes, macOS steps: [references/docker-and-macos.md](references/docker-and-macos.md) (read when env or Docker issues arise).

## Checklist
- [ ] `just precommit` green before commit.
- [ ] Commit message is Conventional.
