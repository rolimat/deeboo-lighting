use tauri_build::{AppManifest, Attributes};

/// Every command the app exposes; the popover and editor capabilities together list exactly these (see `commands` tests).
const COMMANDS: &[&str] = &[
    "get_status",
    "popover_visible",
    "get_settings",
    "get_library",
    "save_library",
    "select_set",
    "select_palette",
    "set_live_set",
    "pattern_sequence",
    "save_settings",
    "list_midi_sources",
    "list_audio_inputs",
    "ui_command",
    "quit",
    "open_editor",
    "wizard_start",
    "wizard_choose",
    "wizard_retry",
    "wizard_state",
    "wizard_cancel",
    "strip_identify",
    "identify_segment",
    "wizard_save",
    "set_wired_segments",
    "message_counts",
    "preview_generated",
    "preview_palette",
    "preview_sequence",
    "calibrate_latency",
    "detection_watch",
    "set_detection_thresholds",
];

fn main() {
    if let Err(e) = tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)),
    ) {
        eprintln!("tauri build failed: {e:#}");
        std::process::exit(1);
    }
}
