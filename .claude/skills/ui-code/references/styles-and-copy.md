# Styles and copy

## Tokens
Same tokens as Deeboo MIDI Sync (spec §8), defined once in `:root` of `ui/src/styles.css`:
`--bg:#1c1d24` `--card:#262833` `--border:#33364a` `--text:#eee` `--dim:#8a8da0`
`--green:#2fb36b` `--green-dark:#1f6b43` `--red:#e5484d`. Radius 12px on cards, 16px on the panel.
Popover width 340px, dark, rounded cards (spec §7.2, approved mockup `menubar-2.html`).

## Editor visual language (redesign spec 4)
- Top bar, always the same: tabs (Sets, Sequences, Palettes, Strip, Detection, Setup, each with a line icon), undo, redo, separator, duplicate, delete (red; only where something is selected), save state, help (?), live readout.
- Save state: "Saving…", "✓ Saved", red "Not saved — <reason>". Live readout: "● 122.0 BPM · beat 2 · phrase: bar 3 of 8 · <icon> Rhythm" or "○ No clock". Delete asks: `Delete "<name>"? You can undo this with ⌘Z.` (Cancel / Delete).
- Label left of its field, same row. Left navigator (search, list) in Sets, Sequences, Palettes.
- One teal accent (`--color-accent`) for live, selection and valid drop targets; red only for delete and errors; no colour per moment. Moment icons: Rhythm pulse, Calm wave, Build-up rising arrow.
- Icons: bundled line SVGs (Lucide, ISC, see `ui/src/icons/LICENSE.md`); no emoji or pictographs anywhere. Undo/redo are disabled on Strip, Detection and Setup with the hint "This tab applies changes at once, so there is nothing to undo."

## Component classes
`.test-strip`: debug-builds-only "Test strip" button in the popover card.

## Strings (English only)
The authoritative copy is in spec §6.7 and §7. Copy that exists in code is listed here by the task that adds it.
- Debug builds only: "Test strip".
- Connection wizard only: "Connect with Smart Life", where to find the User Code (Me, Settings, Account and Security, User Code).
- Paint step: "Open Smart Life, go to the paint screen and tap any segment".
- Wizard (`ui/src/editor/strip/`): "Open Smart Life → Me → Settings → Account and Security → User Code, then paste it here." (`Continue`; invalid: "Enter the User Code from Smart Life (letters and digits only)."), "Scan this code with Smart Life and confirm." (`Cancel`), "Looking for <name> on your wifi…", the 30 s "Not found yet. …" help, the local-network note "macOS may ask to allow Deeboo Lighting to find devices on your local network. Choose Allow.", identify pieces numbered with `Save`, "Connected: <name> (<N> segments).", failures with `Retry` ("No paint data received. The strip may not be compatible.").
- Unsupported devices appear greyed out as "not supported".
- Sequences (`ui/src/editor/sequences/`): roles "Principal", "Contraste", "Suave", "White", "Off"; step lengths "½ beat", "1 beat", "2 beats", "4 beats"; buttons `Add step`, `New sequence`, `Duplicate sequence`, `Delete sequence`, `Save`; warning "Shown in <N> parts" on a step that costs more than one message; name error "Name must be 1 to 63 characters, without control characters."; grid cells are labelled "Segment 3, step 2: Principal". Deleting a sequence a set uses is refused on save and the Rust message is shown.
- Popover: "Lights ▶", "Off", "Rhythm", "Calm", "Build-up", "Auto ✓", "drop predicted", "Brightness", rows "MIDI · <source>", "Strip · <name> (<N> segments)" / "not connected" / "connecting…" / "connection error", "Audio · <device>" / "no access" / "off".
- Settings: "‹ Settings", "MIDI input" ("Virtual port (Deeboo Lighting In)"), "Audio input" ("System default"), "Phrase length" (4 / 8 / 16), "Detection sensitivity", "Light latency (ms)", "Quit".
