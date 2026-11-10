# Deeboo Lighting

A macOS menu bar app that keeps a Tuya LED strip in time with your music. It follows a MIDI clock
for beat, bar and phrase, listens to the audio to detect musical moments (build-ups, drops, breaks)
and paints the strip accordingly.

## Requirements
- macOS on Apple Silicon.
- A Tuya LED strip on your Wi-Fi, and the account of the app that set it up.
- Any MIDI clock source (a DJ app, a DAW or a mixer). Pick it in Setup; sources that start after the app are picked up without a restart.

## Install
Download the `.dmg` and `SHA256SUMS` from the GitHub Releases page. The app is ad-hoc signed but not notarized, so macOS warns the first time.
1. **Verify the download (recommended).** In the folder with both files run `shasum -a 256 -c SHA256SUMS`, or
   `gh attestation verify Deeboo-Lighting_<version>_aarch64.dmg --repo rolimat/deeboo-lighting`.
2. Open the DMG and drag the app to Applications.
3. Right-click the app and choose **Open**, then **Open** again. If macOS only offers Done, go to
   **System Settings → Privacy & Security** and click **Open Anyway**.
4. Fallback if macOS says the app "is damaged": `xattr -dr com.apple.quarantine "/Applications/Deeboo Lighting.app"`

## Permissions
- **Microphone**: to hear the music and pick the lighting moment. Audio is analysed on your Mac and never leaves it.
- **Local network**: to find and control the strip. Only private IPv4 addresses in your Mac's own subnet are used.

## Using it
- **Menu bar**: a single item showing the BPM and a beat indicator. Click it to open the popover.
- **Popover**: a play button to turn the lights on or off, the current moment (or force one, then back to Auto), set and palette (Auto rotates the palette with the set), and Open editor.
- **Editor tabs**: Sets · Sequences · Palettes · Strip · Detection · Setup. Edits save automatically, with undo and redo (⌘Z / ⇧⌘Z) on the library tabs. Sets choose which sequences play in each moment; Sequences are whole-bar patterns you paint or generate; Palettes hold the colours; Strip is the segment layout; Detection shows the live audio lanes and the thresholds for each moment; Setup picks the MIDI source, audio input and light latency.

## Connect the strip
Open the editor from the menu bar item (it opens on Strip when no strip is saved) and follow the wizard. It asks for the User Code from Smart Life
(Me → Settings → Account and Security → User Code) and shows a QR to scan with the Smart Life app.
The cloud session is closed right after the device list is fetched. The strip's local key stays on your Mac,
in its own file with mode 0600 in the app's config folder. It is never logged or sent anywhere else.

## Development
Everything except real MIDI, audio, the strip and the DMG runs in Docker. Requires Docker and `just`.
- `just precommit`: format check, lint and tests (also the git hook).
- `just dev`, `just dmg`: macOS host only. The DMG lands in `target/release/bundle/dmg/`.
- `just ship`: push and wait for CI.
- Releases: pushing a tag `vX.Y.Z` builds the DMG and publishes it with `SHA256SUMS` and a provenance attestation.

Agent guidance: `AGENTS.md` and `.claude/skills/`. Design: `docs/superpowers/specs/`.

## License
GPL-3.0.
