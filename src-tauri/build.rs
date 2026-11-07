use tauri_build::{AppManifest, Attributes};

/// Release manifest: the dev command must never appear here.
const COMMANDS: &[&str] = &["get_status"];
const DEV_COMMANDS: &[&str] = &["get_status", "dev_paint_test", "dev_mic_level"];

fn main() {
    let commands = if std::env::var("PROFILE").as_deref() == Ok("debug") {
        DEV_COMMANDS
    } else {
        COMMANDS
    };
    if let Err(e) = tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(commands)),
    ) {
        eprintln!("tauri build failed: {e:#}");
        std::process::exit(1);
    }
}
