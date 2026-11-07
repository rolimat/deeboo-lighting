# Useful commands

```sh
just precommit   # fmt-check + lint + tests, inside Docker (the pre-commit hook runs this)
just fmt         # format Rust (+ UI once ui/ exists)
just test        # Rust (+ Vitest once ui/ exists) tests, in Docker
just lint        # clippy -D warnings, cargo-deny, hadolint, gitleaks, agent-docs budget (+ eslint, tsc)
just dev         # macOS host: run the app (needs src-tauri/ and ui/)
just ship        # push and wait for the CI run of this exact commit
```

# TDD (mandatory)

- Every behaviour change starts with a failing test: write it, run it, see it fail (red),
  then write the minimum code to pass (green), then refactor.
- No production code without a test that exercised it first. Bug fixes start with a test that reproduces the bug.
- Where tests go and how (simulated time, fixed DP 61 payloads, mocked UI api): `writing-tests` skill.

# Architecture

- macOS menu bar app: Tauri 2 + Rust; syncs Tuya LED strips to MIDI clock. UI in TypeScript + Vite, plain CSS.
- `crates/light-core/`: pure, deterministic, no I/O, simulated time. Clock to beat/bar/phrase, moment state
  machine, model (sequences, palettes, sets), generator, scene diff to commands, scheduler with spacing/drops,
  DP 61 encoding, moment classifier from per-beat audio features.
- `src-tauri/`: midir, audio capture, rustuya, cloud login, credentials, persistence, tray, IPC.
- `ui/`: popover + editor window. `tools/tuya-login/`: dev CLI for the Smart Life QR login. `tools/detect-probe/` (later): offline detector on a recording.
- Everything except real MIDI, audio, the strip and the DMG runs in Docker.

# Golden rules

- App text is English. "Smart Life" is named only in the connection flows.
- Secrets (`local_key`, cloud tokens, User Code, QR token) live only in `secrecy::SecretString`: never
  printed, logged or committed. `local_key` goes in its own file, mode 0600, apart from all other data.
- Cloud: HTTPS only, host allowlist. LAN: only RFC 1918 IPv4 in the Mac's own subnet.
- Webview loads only bundled files, strict CSP, minimal Tauri capabilities; all network I/O is in Rust.
- Event-driven: no polling loops, no busy waits. No allocations, locks or logs in the audio callback.
- Audio features come from IIR band filters and envelopes before any FFT; FFT only if tuning proves it indispensable.
- Never `--no-verify`. Conventional Commits. Never commit recordings, labels or dev keys.

# Documentation

- Spec (Spanish, the authority): `docs/superpowers/specs/2026-11-06-deeboo-lighting-design.md`
- Detailed conventions live in skills. Read only what your task needs
  (Claude Code loads them automatically; other agents open the `SKILL.md` first):

```
.claude/skills/
├── rust-engineer/          # Rust (original Jeffallan skill, unchanged; see SOURCE.md)
├── security/               # threat model + per-PR checklist (spec §12); read for any secret, network, IPC, CSP or dependency change
├── tuya-protocol/          # DP 61 format, fixtures, limits, Smart Life QR login (spec §5.2-5.5)
├── writing-tests/          # TDD, simulated time, fixtures, Vitest with mocked api
│   └── references/{rust-simulated-time.md, ui-vitest.md}
├── ui-code/                # anything in ui/: views, styles, copy
│   └── references/styles-and-copy.md
└── dev-workflow/           # Docker vs macOS, hooks, CI, releases
    └── references/docker-and-macos.md
```

# Plans

- New feature or behaviour change: brainstorm first, then write the spec in `docs/superpowers/specs/`
  and the plan in `docs/superpowers/plans/YYYY-MM-DD-<topic>.md`.
- Specs are committed. Plans are never committed (`docs/superpowers/plans/` is git-ignored).
- **Plan-only prompts** ("create a plan for...", "crea un plan para..."): only write the plan. No code, no app runs.
- **Implement** only when explicitly asked ("implement the plan", "implementa el plan").

# Git

- Work on a feature branch and open a PR to `main`; push to `main` only when the user says so.
- Ask before pushing any tag.

# Done means

- Tests written first (red, then green) and `just precommit` green.
- The PR passed the checklist in the `security` skill.
- Any change to a convention, command or interface updates its skill in the same commit.
