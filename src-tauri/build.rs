use tauri_build::{AppManifest, Attributes};

/// Release manifest. Task 9 adds a `PROFILE == "debug"` branch selecting a longer list
/// (`get_status` + `dev_paint_test`); release builds must keep this list.
const COMMANDS: &[&str] = &["get_status"];

fn main() {
    if let Err(e) = tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)),
    ) {
        eprintln!("tauri build failed: {e:#}");
        std::process::exit(1);
    }
}
