# App Sandbox check (Plan 1, Task 10)

> Historical record. The debug probes it used (`Test strip`, `Mic level`, the MIDI tick counter, the
> `dev_*` commands) were removed in Plan 3 Task 8; the app now reads its strip from
> `<app_config_dir>/strip.json` (`just install-strip`) and listens to MIDI through the engine thread.

Checks that the sandboxed debug bundle can do what the app needs. If any item fails, stop and
ask (spec §12.2): never relax the sandbox on your own.

## Procedure

1. Build: `npm --prefix ui run tauri build -- --debug --bundles app`
2. `codesign -d --entitlements - <bundle>` lists sandbox, network client, network server, audio-input.
3. Launch the bundle once (`open -n <bundle>`) so macOS creates the container, then quit it.
4. Copy the strip file (the sandbox cannot read the repo's `.dev/`):
   `install -m 0600 .dev/strip.json ~/Library/Containers/com.deeboo.lighting/Data/strip.json`
5. Start Deeboo MIDI Sync first (the probe listens to the MIDI sources present at launch), then run the app
   **through `open`**, not from a terminal: macOS attributes the Local Network and microphone permissions to
   the process that launches it, so a terminal launch would test the terminal's permissions instead:
   `open -n --env DEEBOO_DEV_STRIP=$HOME/Library/Containers/com.deeboo.lighting/Data/strip.json --env RUST_LOG=info --stdout <log> --stderr <log> <bundle>`
6. Answer the Local Network and microphone prompts with Allow.
7. When done, delete the copied strip file from the container.

## Results — 2026-11-07, macOS 26.4, ad-hoc signed debug bundle with hardened runtime

| Item | Check | Result | Notes |
| ---- | ----- | ------ | ----- |
| (a1) | MIDI clock from a source: the app listens to Deeboo MIDI Sync's output (`Deeboo Clock Sync Out`); `MIDI clock · N ticks` rises | Pass | 4 728 ticks counted while MIDI Sync played |
| (a2) | MIDI clock to the virtual destination: another process lists `Deeboo Lighting In` and sends to it (MIDI Sync stopped) | Pass | Counter 0 → exactly 480 after sending 480 clocks with a midir test sender |
| (b) | Discovery finds the strip (popover shows it connected) | Pass | First run timed out until the user allowed Local Network for the app; then connected in 29 s and 3 s |
| (c) | `Test strip` paints | Pass | Odd red → even white → segment 1 off → all blue |
| (d) | Microphone prompt appears; `Mic level` shows `Mic · X dBFS` with sound | Pass | Prompt shown; −19.6 dBFS with music |

No entitlement was added or relaxed. Launch at login is out of scope (spec §3, user 2026-11-07).
