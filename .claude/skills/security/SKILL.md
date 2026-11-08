---
name: security
description: Use when touching secrets, credentials, cloud login, LAN/network code, Tauri IPC or capabilities, CSP, config loading, dependencies, CI workflows, or reviewing any PR. Threat model and per-PR checklist.
---

# Security

Spec §12 (mandatory from the first task). Update the threat model when a new surface appears.

## Threat model
| Asset | Why it matters | Handling |
|---|---|---|
| Cloud session (`access_token`, `refresh_token`) | Whole Smart Life account: every device and key | Memory only; revoked when the wizard ends or is cancelled |
| `local_key` | Controls the strip from the LAN | Own file, mode 0600; never in logs, UI, exports, repo |
| User Code, QR token | Identify account and login session | Never stored; never logged |
| Microphone audio | Room privacy | Memory only; never recorded or sent |
| Distributed app | Integrity | Verifiable release (SHA256SUMS + provenance) |

Attackers: another LAN device (fake discovery, tampered messages), tampered network responses, edited config
files, compromised dependencies, malicious webview content. Out of scope: malware already running as the user,
physical access. Accepted risk: Tuya 3.3 uses AES-ECB with a fixed key (weak, device-imposed).

## Checklist for every PR
Cloud and secrets
- [ ] HTTPS only, system TLS; hosts limited to `apigw.iotbing.com` and the allowlisted Tuya domains. The login
      `endpoint` is accepted only if `https` and on that allowlist; anything else is rejected.
- [ ] Timeouts and response size limits on every request.
- [ ] After reading the device list, the session is revoked (`POST /v1.0/m/token/terminal/expire`) and dropped
      from memory; same on cancel.
- [ ] Secrets are wrapped in `secrecy::SecretString` (or equivalent with `zeroize`): Debug/Display redacted, wiped on drop.
      UI shows the key masked.
- [ ] No log contains key, tokens, User Code or QR token; a test checks this.
- [ ] `local_key` file is 0600 and separate from the library; the key never enters the JSON config.
Local network
- [ ] Connect only to RFC 1918 IPv4 in the Mac's own subnet.
- [ ] Discovery announcements count only when the ID matches the chosen strip.
- [ ] Network input is untrusted: size limits, errors instead of panics.
App and webview
- [ ] Webview loads only bundled files; strict CSP (no inline, no external origins). QR is generated locally.
- [ ] Minimal Tauri capabilities per window: own commands only; no shell, fs or http plugin exposed to JS.
      All network I/O happens in Rust.
- [ ] Every IPC input is validated in Rust: types, ranges (segments, DP 61 <= 255 bytes, steps, H/S/V, BPM), text lengths.
- [ ] Config and library validated on load (schema version, limits). Invalid file: keep a copy, start with
      defaults, no data loss, no crash.
- [ ] MIDI parser accepts only the real-time clock bytes and ignores the rest.
Rust
- [ ] `#![forbid(unsafe_code)]` in our crates; `clippy::unwrap_used` / `expect_used` denied outside tests.
- [ ] Property tests (`proptest`) for every parser: DP 61, MIDI, config, cloud responses.
macOS
- [ ] Hardened Runtime + App Sandbox with minimal entitlements (network client, network server for UDP discovery,
      audio input). Check early that the virtual MIDI port works in the sandbox; if not, stop and ask.
- [ ] No auto-updater. Info.plist has `NSMicrophoneUsageDescription` and `NSLocalNetworkUsageDescription`.
Supply chain and repo
- [ ] `cargo-deny` passes (advisories, licences, crates.io only, bans); `npm audit --audit-level=high` in CI;
      lockfiles committed, `npm ci`; minimal dependencies (UI runtime: only `@tauri-apps/api`).
- [ ] GitHub Actions pinned by commit SHA, read-only `permissions` by default, publish separate from build,
      no `pull_request_target`, Dependabot on.
- [ ] `gitleaks` clean (pre-commit and CI) and it actually scanned ("N commits scanned", N > 0 for `just lint`); the recipes fail closed when git cannot read the repo. Recordings, labels and dev keys are git-ignored.
- [ ] `/security-review` run before each release.

Dev-only key loading via environment variable exists only in debug builds and has no UI.
