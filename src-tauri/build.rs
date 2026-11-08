use tauri_build::{AppManifest, Attributes};

/// Every command the popover may call; the capability lists exactly these (see `commands` tests).
const COMMANDS: &[&str] = &[
    "get_status",
    "get_settings",
    "save_settings",
    "list_midi_sources",
    "list_audio_inputs",
    "ui_command",
    "quit",
];

fn main() {
    if let Err(e) = tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)),
    ) {
        eprintln!("tauri build failed: {e:#}");
        std::process::exit(1);
    }
}
