# Security review: Deeboo Lighting v0.1.0

- **Date:** 2026-11-08
- **Branch / commit reviewed:** `feat/plan-7-release` at `b38905c`
- **Scope:** the whole tree as it ships (not a diff): `src-tauri/`, `crates/light-core/`, `crates/tuya-cloud/`,
  `ui/`, `tools/` (dev only, not shipped), build and CI files (`Cargo.toml`, `deny.toml`, `justfile`, `lefthook.yml`,
  `docker/`, `.github/`), `Entitlements.plist`, `Info.plist`, `tauri.conf.json`, capabilities, `scripts/log-scan.sh`.
- **Authority:** `.claude/skills/security/SKILL.md` (threat model and checklist), spec §12
  (`docs/superpowers/specs/2026-11-06-deeboo-lighting-design.md`), `CLAUDE.md` golden rules.

## Method

`/security-review` reviews the pending diff, so the same criteria were applied by hand to the whole tree. Read in
full: every file in `src-tauri/src/` (wizard, commands, strip, strip store, settings, library store, LAN policy,
MIDI, audio, QR, editor window, logger setup), `crates/tuya-cloud/src/` (login, transport, crypto), the library
validation in `crates/light-core/src/library.rs`, the MIDI parser in `crates/light-core/src/clock.rs`, the UI DOM
helpers and every UI file for HTML sinks, `tools/tuya-login/`, all config, CI and release files. The log calls of the
vendored `rustuya-tokio 0.4.6` and the proxy default of `ureq 3.5.1` were read in `~/.cargo/registry`.

Commands run and results:

| Command | Result |
|---|---|
| `docker compose run --rm dev gitleaks git --no-banner --redact` | `117 commits scanned`, `no leaks found` |
| same with `--log-opts="--all"` | `117 commits scanned`, `no leaks found` (`git rev-list --all --count` = 125; the rest are stash, merge and Dependabot refs without new blobs) |
| `docker compose run --rm dev cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` (only `duplicate` warnings) |
| `npm --prefix ui audit --audit-level=high` | exit 0; 10 **moderate** advisories, all in the dev-only eslint chain (`minimatch` GHSA-g3hv-75wg-m249); nothing shipped |
| `npm --prefix ui ls --omit=dev --depth=0` | runtime deps: `@tauri-apps/api@2.12.4`, `preact@11.2.0` |
| `grep -rn innerHTML\|insertAdjacentHTML\|dangerouslySetInnerHTML\|eval(\|new Function\|window.open ui/src` | no hits outside tests |
| `grep "<script" ui/dist/*.html` | only external module scripts from `/assets/` |
| `scripts/log-scan.sh` on a log line `local_key=k$y1234567890ab` | exit 0 (missed; see F3) |
| `grep -rln proptest` | DP 61, MIDI clock, scene, groups, patterns, cloud responses, strip frames; no config parser (see F5) |

## Checklist

| # | Item | Status | Evidence |
|---|---|---|---|
| C1 | HTTPS only, system TLS, no redirects | pass | `crates/tuya-cloud/src/transport.rs:123-137` (`https_only`, `PlatformVerifier`, `max_redirects(0)`) |
| C2 | Login host fixed; `endpoint` only `https` + allowlisted suffix | pass | `login.rs:18-26`, `login.rs:228-245`, called at `login.rs:218` |
| C3 | Timeouts and response size limits | pass | `transport.rs:79-80` (1 MiB, 10 s), `transport.rs:171-175` |
| C4 | Session revoked after the device list and on cancel | pass, with F2 | `wizard/cloud.rs:139-169` (cancel in flight, list then revoke in one closure); `lib.rs:65-72` (editor closed), `lib.rs:95-100` + `commands.rs:398-401` (quit) |
| C5 | Secrets in `SecretString`, Debug redacted, wiped on drop | finding F1 | Types: `login.rs:30-92`, `login.rs:104-132`, `strip_store.rs:13-30`; plain copies: see F1 |
| C6 | UI never shows the key | pass | the key never crosses IPC: `wizard/state.rs:18-28` (`DeviceDto` has no key), `strip_store.rs` is not reachable from any command |
| C7 | No log contains secrets; a test checks it | pass, with F3 | all `log::` calls reviewed; network stacks capped at `warn` (`lib.rs:24-29`, test `lib.rs:113-125`); capturing logger `wizard/mod.rs:9-45` used by 3 tests |
| C8 | `local_key` file 0600, atomic, separate from library and settings | pass | `strip_store.rs:103-171` (mode check on load, `create_new` + `mode(0o600)` + `sync_all` + rename, dir 0700) |
| C9 | Only RFC 1918 IPv4 on the Mac's own subnet | pass | `lan.rs:6-31`; used by discovery `strip.rs:204-217`, saved address `strip.rs:112-114`, listen `strip.rs:358-360` |
| C10 | Discovery counts only the chosen ID | pass | `strip.rs:214-223` |
| C11 | Network input: size limits, no panics | pass | `strip.rs:235-245` (`MAX_FRAME` 4096, segment cap 60), proptest `strip.rs:811-817`; cloud JSON via `serde_json` with typed fields, `login_tests.rs` proptests |
| C12 | Webview loads bundled files only; strict CSP; QR generated locally | pass | `tauri.conf.json:9-10,30` (`default-src 'self'`, no `unsafe-inline` in release); `qr.rs:27-37`; no HTML sinks in `ui/src` |
| C13 | Minimal capabilities per window; no shell/fs/http plugin | pass | `capabilities/popover.json`, `capabilities/editor.json` (wizard commands only in the editor; `set-size` only in the popover); test `commands.rs` `release_manifest_has_no_dev_commands`; `withGlobalTauri: false` |
| C14 | Every IPC input validated in Rust | pass | `Settings::validate` `settings.rs:71-96`; `Thresholds` integer + range; `UiCommandDto`/`MomentDto`/`Section` closed enums; `select` delta ±1 `commands.rs:248`; library `validate` + Rust-assigned ids `commands.rs:285-301`; `message_counts` step cap `commands.rs:499`; segment ranges `commands.rs:643-649`, `session.rs:332`; User Code `login.rs:35-42`; device id `session.rs:244`, `strip_store.rs:56-58` |
| C15 | Config and library validated on load; invalid file kept aside, defaults, no crash | pass for library and strip; finding F4 for settings | `library_store.rs:13,24-30,63-80` (4 MiB cap, schema version, `.bad` rotation); `settings.rs:101-134` |
| C16 | MIDI parser takes only real-time clock bytes | pass (note N4) | `crates/light-core/src/clock.rs:111-118`; callback `midi.rs:27-36` |
| C17 | `forbid(unsafe_code)`, `unwrap`/`expect`/`panic` denied | pass | `Cargo.toml:11-18`; every member has `[lints] workspace = true` |
| C18 | Property tests for every parser (DP 61, MIDI, config, cloud) | finding F5 | DP 61 `tests/paint_fixtures.rs`, MIDI `tests/clock.rs`, cloud `login_tests.rs`, strip frames `strip.rs:811`; none for settings/library/strip file |
| C19 | Hardened Runtime + App Sandbox, minimal entitlements | pass | `Entitlements.plist:5-12` (sandbox, network client, network server, audio input; no `cs.*` exceptions); `tauri.conf.json:44-48` |
| C20 | No auto-updater; usage strings present | pass | no updater plugin in `src-tauri/Cargo.toml`; `Info.plist:7-10` |
| C21 | `cargo-deny`, `npm audit --audit-level=high`, lockfiles, `npm ci`, minimal deps | pass | `justfile:31,37`; `deny.toml` (`unknown-registry`/`unknown-git` deny, wildcards deny); `npm ci` in `.github/actions/macos-build/action.yml:13`; `--locked` everywhere. Preact is a second UI runtime dep by recorded decision (Plan 4) |
| C22 | Actions pinned by SHA, read-only default permissions, publish separate, no `pull_request_target`, Dependabot | pass, with F6 | `ci.yml:8-9`, `release.yml:7,12-13,28-31`; all `uses:` pinned; `.github/dependabot.yml` covers cargo, npm, actions, docker |
| C23 | gitleaks clean, really scans, fails closed | pass | `justfile:35-36,92-93`; `docker/Dockerfile:58-59` (`safe.directory`); 117 commits scanned |
| C24 | Recordings, labels, dev keys git-ignored | pass | `.gitignore:7-8` (`.dev/`, `recordings/`); no media or key files tracked or ever added |
| C25 | Acceptance session log scanned clean | pending (acceptance task) | `scripts/log-scan.sh`; see F3 for a gap in its patterns |
| C26 | Dev-only key loading only in debug builds, no UI | pass | `strip.rs:54-63` (`#[cfg(debug_assertions)]`, `DEEBOO_DEV_STRIP`) |
| C27 | Release verifiable (SHA256SUMS + provenance) | pass | `release.yml:37-46`; `README.md:13-19` |
| C28 | No allocations, locks or logs in the audio and MIDI callbacks | pass | `audio.rs:101-106`, `midi.rs:27-36` |

## Findings

No HIGH or MEDIUM findings. All findings below have confidence of 0.7 or more.

### F1 · LOW · confidence 0.9 · Plain copies of secrets are not wiped

**Where.**
- `crates/tuya-cloud/src/login.rs:170-173` and `203-207`: the request URL `String` holds the User Code and the QR token.
- `login.rs:315-325`: the `X-token` header copies the access token into a plain `String`; `HttpRequest` (`transport.rs:89-95`) has no zeroising fields.
- `login.rs:385-388`: the revoke body is a `serde_json::Value` with the access token and terminal id.
- `transport.rs:171-175` returns the poll response as a plain `String`. `login.rs:208-224` parses it into a `Value`. The access and refresh tokens therefore sit unwiped in two heap buffers; `crypto::hash_key` (`crypto.rs:32-37`) also returns a plain `String`.
- `src-tauri/src/strip_store.rs:112` reads the file into a plain `Vec<u8>`; `strip_store.rs:143-151` builds the JSON `Value` and bytes with the key.
- `src-tauri/src/strip.rs:126` and `361`: `to_vec()` of the key is handed to `rustuya`.

**Scenario.** The session tokens and the key stay in freed heap memory after the wizard ends. They could then reach
swap, a crash report or a core dump. Reading process memory directly is out of scope (malware running as the user),
so the impact is low. Even so, the spec (§12.2, "se borran de memoria al soltarse") and checklist C5 promise a wipe.

**Fix.** Make `HttpRequest.url`, `headers` values and `body` `Zeroizing<String>`, and have `Transport::send` return
`Zeroizing<String>`. Parse the poll response through a `#[derive(Deserialize)]` struct whose token fields are
`SecretString`, as `CloudDevice` already does. Build the revoke body and the strip file bytes as `Zeroizing`, and read
the strip file into `Zeroizing<Vec<u8>>`. The key copy given to `rustuya` is out of our hands; record it as accepted.
Test: a unit test that each new type is `Zeroizing`/`SecretString` (compile-time) plus the existing log tests.

### F2 · LOW · confidence 0.75 · Quitting during the device list can skip the revoke

**Where.** `src-tauri/src/commands.rs:22` (`QUIT_WAIT` = 5 s), `commands.rs:398-401`, `session.rs:304-316`.
Per-request timeout: `transport.rs:80` (10 s). The list-then-revoke closure is in `wizard/cloud.rs:157-163`:
`list_devices` makes 1 + one per home requests (`login.rs:351-377`), then `revoke` makes one more.

**Scenario.** The user scans the QR, then quits from the popover while `list_devices` runs on a slow network.
`shutdown` waits 5 s and the process exits. The `spawn_blocking` thread dies before `POST /v1.0/m/token/terminal/expire`
is sent. A full-account session stays valid on Tuya's side until it expires by itself. The "revoked on quit" promise
only holds when the cloud answers fast.

**Fix.** On shutdown, skip the remaining `list_devices` calls and revoke at once. For example, check a cancel flag
between home requests and give revoke its own short timeout. Alternatively, make `QUIT_WAIT` cover the worst case of
one in-flight request plus revoke (more than 20 s). Test with the fake transport: a slow `list_devices` and a
`shutdown` must still see the revoke request.

### F3 · LOW · confidence 0.9 · `log-scan.sh` misses local keys with punctuation and bare keys

**Where.** `scripts/log-scan.sh:7-10`; fixture `scripts/tests/log-scan/dirty.log` only uses alphanumeric values.

**Scenario.** Tuya local keys are 16 printable characters and often contain `$`, `|`, `<`, `'` or `;`. The named-field
pattern needs `[A-Za-z0-9_-]{6,}` right after the separator, and the bare pattern needs 32 or more alphanumerics.
So `local_key=k$y1234567890ab`, or a bare 16-char key printed by a dependency, passes the scan (checked: exit 0). The
code review found no such log line, but the acceptance scan (C25) would not catch a regression.

**Fix.** Widen the value class to anything up to a delimiter (`[^[:space:]\"',}]{6,}`). Also add a fixed-string
check against the real key: let `just log-scan` optionally read the installed `strip.json` and
`grep -F -- "$key"`, printing line numbers only. Add punctuated-key lines to `dirty.log`.

### F4 · LOW · confidence 0.85 · `settings.json` loader lacks the library loader's guards

**Where.** `src-tauri/src/settings.rs:101-126` and `13-15`.

**Scenario.** The threat model counts edited config files. `load` uses `std::fs::read_to_string` with no cap, so a
huge or sparse file is read fully into memory at every start. The library loader caps files at 4 MiB
(`library_store.rs:13,24-30`); the settings loader does not. The file also has no schema version, while spec §12.2
asks for "versión de esquema y límites". Finally, `settings.rs:121` logs the `serde_json` message, which echoes the
offending value. `library_store.rs:32-35` avoids that on purpose. Settings hold no secrets, so this only means
device names in the log.

**Fix.** Reuse a capped read (64 KiB is plenty). Add `schemaVersion` with the same default-on-missing migration the
strip file uses. Log `classify()`, line and column only. Test: an oversized file loads defaults and is kept as `.bad`;
a wrong version does the same.

### F5 · LOW · confidence 0.9 · No property tests for the config parsers

**Where.** No `proptest` in `settings.rs`, `library_store.rs`, `strip_store.rs` or `crates/light-core/tests/library.rs`.
The spec (§12.2) and C18 require them for "configuración".

**Scenario.** Hand-written cases cover the known bad inputs, but nothing checks that arbitrary bytes or arbitrary
valid-JSON shapes never panic and always end in defaults plus a `.bad` copy. That is the "no data loss, no crash"
promise.

**Fix.** Add proptests: arbitrary bytes into `settings::load`, `library_store::load` and `strip_store::load` (0600)
must not panic. Settings and library must return defaults and keep the original aside; the strip loader must return
`Err`.

### F6 · LOW · confidence 0.8 · The release publishes on any `v*` tag without the quality gates

**Where.** `.github/workflows/release.yml:3-47`. The build job runs no tests, clippy, `cargo deny`, `npm audit` or
gitleaks, and nothing requires CI to have passed for the tagged commit.

**Scenario.** A tag pushed on a commit whose CI failed (or never ran, or a branch commit) publishes a DMG with valid
provenance. The attestation then vouches for an unreviewed build. Today the only safeguard is the process rule "ask
before pushing any tag".

**Fix.** Gate the release on the gates: either a first job running `docker compose run --rm dev just precommit` (as
`ci.yml:57`) plus `cargo test` on macOS, or a job that checks the CI run of `github.sha` passed and the commit is on
`main` (`gh api repos/:owner/:repo/commits/$SHA/check-runs`). Also add a tag ruleset limiting who can create `v*` tags.

### F7 · INFO · confidence 0.9 · The cloud client honours proxy environment variables

**Where.** `transport.rs:123-137`; `ureq 3.5.1` `Config::default` sets `proxy: Proxy::try_from_env()` (`ALL_PROXY`,
`HTTPS_PROXY`, `HTTP_PROXY`).

**Scenario.** If the app is started from a shell with a proxy set, cloud traffic goes through it. TLS is still
end-to-end and verified, so the proxy sees only host names and timing. A Finder launch has no such variables.

**Fix.** Add `.proxy(None)` to the agent config so the allowlisted hosts are always dialled directly.

### F8 · INFO · confidence 0.9 · Stale statements in CI and the security skill

**Where.** `ci.yml:15` and `ci.yml:23` say "the repo is private", but it is public. Security skill C21 says "UI
runtime: only `@tauri-apps/api`", but the editor ships Preact by the recorded Plan 4 decision.

**Fix.** Update the comments, and the skill line to "`@tauri-apps/api` and `preact` (pinned)".

## Notes (confidence below 0.7, or accepted by design)

- **N1 · Session not revoked when the login answer is rejected.** If `poll_login` gets a session whose `endpoint` is
  not allowlisted, or a field is missing (`login.rs:218-224`), the tokens are dropped without a revoke. That is
  deliberate: nothing is sent to an untrusted host. The session expires on Tuya's side.
- **N2 · Fake discovery.** A LAN host in the Mac's subnet can announce the strip's ID with its own address. The app
  then dials it. Without the key it learns nothing usable (3.3 AES-ECB, accepted risk), but it can answer with an auth
  failure and make the popover show "key invalid" (a nuisance-level DoS). This is within the accepted threat model.
- **N3 · `rustuya` trace logs.** With `RUST_LOG=trace`, `rustuya-tokio` logs decrypted rx payloads (DPS JSON) and tx
  ciphertext (`actor.rs:488,539`); no key and no token. Capping `rustuya_tokio` at `debug` in `lib.rs:24-29` would keep
  user debug sessions smaller.
- **N4 · MIDI callback forwards every status byte.** `midi.rs:27-36` queues every message; only the engine ignores
  non-clock bytes. A dense non-clock stream (MPE, sysex) on the chosen source competes with clock ticks and UI inputs
  for the 1024-slot queue. Filtering `0xF8/0xFA/0xFB/0xFC` in the callback would keep the queue for clock.
- **N5 · CSP extras.** `img-src` lists `asset:`/`asset.localhost`, but the asset protocol is not enabled, and
  `base-uri` is not set. There is no injection sink, so this is hardening only.
- **N6 · IPC payload size.** `save_library` deserialises unbounded vectors before `validate`. The webview is bundled
  and has no HTML sink, so only a compromised bundle could abuse it.
- **N7 · Distribution.** The DMG is ad-hoc signed and not notarised (`tauri.conf.json:45`). Integrity rests on
  `SHA256SUMS` and the attestation, which the README documents. `macOSPrivateApi` is on (transparent popover); that
  blocks only App Store distribution.
- **N8 · Dev tool.** `tools/tuya-login` skips revoke on Ctrl-C (documented `shortcut:` at `main.rs:64-65`). It is not
  shipped.
- **N9 · Dependencies.** The 10 moderate npm advisories are dev-only (eslint chain). `deny.toml` allows GPL-3.0, which
  matches the project licence.
