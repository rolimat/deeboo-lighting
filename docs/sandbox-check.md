# App Sandbox check (Plan 1, Task 10)

Checks that the sandboxed debug bundle can do what the app needs. If any item fails, stop and
ask (spec §12.2): never relax the sandbox on your own.

## Procedure

1. Build: `npm --prefix ui run tauri build -- --debug --bundles app`
2. `codesign -d --entitlements - <bundle>` lists sandbox, network client, network server, audio-input.
3. Launch the bundle once so macOS creates the container, then quit it.
4. Copy the strip file (the sandbox cannot read the repo's `.dev/`):
   `cp .dev/strip.json ~/Library/Containers/com.deeboo.lighting/Data/strip.json && chmod 0600 ~/Library/Containers/com.deeboo.lighting/Data/strip.json`
5. Run: `DEEBOO_DEV_STRIP=$HOME/Library/Containers/com.deeboo.lighting/Data/strip.json RUST_LOG=info "<bundle>/Contents/MacOS/deeboo-lighting"`

## Results

| Item | Check | Result | Notes |
| ---- | ----- | ------ | ----- |
| (a) | `Deeboo Lighting In` is a MIDI destination in another app; `MIDI clock · N ticks` rises when Deeboo MIDI Sync sends to it | | |
| (b) | Discovery finds the strip (popover shows it connected) | | |
| (c) | `Test strip` paints | | |
| (d) | Microphone prompt appears; `Mic level` shows `Mic · X dBFS` with sound | | |
