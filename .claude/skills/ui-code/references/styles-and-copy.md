# Styles and copy

## Tokens
Same tokens as Deeboo MIDI Sync (spec §8), defined once in `:root` of `ui/src/styles.css`:
`--bg:#1c1d24` `--card:#262833` `--border:#33364a` `--text:#eee` `--dim:#8a8da0`
`--green:#2fb36b` `--green-dark:#1f6b43` `--red:#e5484d`. Radius 12px on cards, 16px on the panel.
Popover width 340px, dark, rounded cards (spec §7.2, approved mockup `menubar-2.html`).

## Component classes
`.test-strip`: debug-builds-only "Test strip" button in the popover card.

## Strings (English only)
The authoritative copy is in spec §6.7 and §7. Copy that exists in code is listed here by the task that adds it.
- Debug builds only: "Test strip".
- Connection wizard only: "Connect with Smart Life", where to find the User Code (Me, Settings, Account and Security, User Code).
- Paint step: "Open Smart Life, go to the paint screen and tap any segment".
- Wizard (`ui/src/editor/strip/`): "Open Smart Life → Me → Settings → Account and Security → User Code, then paste it here." (`Continue`; invalid: "Enter the User Code from Smart Life (letters and digits only)."), "Scan this code with Smart Life and confirm." (`Cancel`), "Looking for <name> on your wifi…", the 30 s "Not found yet. …" help, the local-network note "macOS may ask to allow Deeboo Lighting to find devices on your local network. Choose Allow.", identify pieces numbered with `Save`, "Connected: <name> (<N> segments).", failures with `Retry` ("No paint data received. The strip may not be compatible.").
- Unsupported devices appear greyed out as "not supported".
- Popover: "Lights ▶", "Off", "Rhythm", "Calm", "Build-up", "Auto ✓", "drop predicted", "Brightness", rows "MIDI · <source>", "Strip · <name> (<N> segments)" / "not connected" / "connecting…" / "connection error", "Audio · <device>" / "no access" / "off".
- Settings: "‹ Settings", "MIDI input" ("Deeboo Lighting In only"), "Audio input" ("System default"), "Phrase length" (4 / 8 / 16), "Detection sensitivity", "Light latency (ms)", "Quit".
