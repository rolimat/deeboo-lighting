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
- Sequences (`ui/src/editor/sequences/`): one Arrangement-style editor. Browser grouped by moment (Rhythm, Calm, Build-up, each with its icon) listing the sequences, then one entry per generated pattern (names from Rust `Pattern::label()`) and "Any pattern", marked with the teal `generated` icon; `New sequence`; palettes to drag onto the grid. Header `Name`, `Moment` (icons), `Preview palette` (UI only, not saved). Clip bar "<name> · <N> bars · loops" with a drag handle (whole bars, 1 to 16; new bars copy the last), ruler "Bar 1, 1.2, ...", rows S1..SN of half-beat cells, paint chips "Principal", "Contrast", "Soft", "Off", zoom out/in. Generated entries open read only with `Duplicate to edit`. Warnings: "Shown in <N> parts" on the column; name error "Name must be 1 to 63 characters, without control characters."; "Used by <set> (<Moment>). It will be removed there." (confirm `Change moment`); "Used by <sets>. Remove it from there first." when deleting; "At most 64 sequences." Cells are labelled "Segment 3, 1.2: Principal". Cells (`cells.ts`) are the editing model: `toCells`/`fromCells`/`setBars`; steps are rebuilt on every edit and never cross a bar.
- Sets (`ui/src/editor/sets/`): browser "Sets" (play icon per set, teal when live; click the name opens it), `New set`, then Sequences by moment (grip, moment icon, teal generated icon), then Palettes with swatches. Header `Set name`, right side "Playing live" or "Not playing · Play live". Tracks Rhythm, Calm, Build-up: "Order" (In order, Random, Fixed icons), "Change every" (1 to 16 bars; Calm also "once per break"), clips "<name>" with "<N> bar(s)" or "auto", hold-to-preview and "Remove from set"; zone "Drop sequences here", while dragging `Drop "<name>" here` or "<Moment> only" / "Sequences only". Palette row "Palettes · change" ("at each drop", "every N bars") + order, chips, zone "Drop palettes here" / "Palettes only". Notices: "A track needs at least one sequence.", "A set needs at least one palette.", "You need at least one set.", "At most 16 sets." New set: "New set".
- Palettes (`ui/src/editor/palettes/`): browser with 3-colour swatches (each swatch drags as a colour) and `New palette`; `Name`; three role cards "Principal · Main colour · most segments", "Contrast · Accents and hits", "Soft · Background, calm moments" (click edits, drag one onto another swaps, drop a browser swatch copies it); `Editing: <role>`, hex field (error "Enter a colour like #FF2D55."), `Brightness · N %` slider (sets only V), "Quick colours", the hue/saturation wheel (`ColourWheel`, maths in `colour.ts`, one undo step per drag); preview with `Sequence:` select and the `Show on strip` toggle; "Used by:" set chips (click goes to Sets) or "No set uses this palette."; delete blocked with "Used by <sets>. Remove it from there first." before any confirmation (`deleteGuard`); "At most 32 palettes.".
- Popover: "Lights ▶", "Off", "Rhythm", "Calm", "Build-up", "Auto ✓", "drop predicted", "Brightness", rows "MIDI · <source>", "Strip · <name> (<N> segments)" / "not connected" / "connecting…" / "connection error", "Audio · <device>" / "no access" / "off".
- Settings: "‹ Settings", "MIDI input" ("Virtual port (Deeboo Lighting In)"), "Audio input" ("System default"), "Phrase length" (4 / 8 / 16), "Detection sensitivity", "Light latency (ms)", "Quit".
