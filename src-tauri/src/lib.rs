//! Deeboo Lighting: menu bar app shell and IPC.

mod commands;
mod tray;

/// Starts the tray app.
///
/// # Errors
/// Returns the Tauri error if the runtime fails to start.
pub fn run() -> tauri::Result<()> {
    env_logger::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![commands::get_status])
        .setup(tray::setup)
        .run(tauri::generate_context!())
}
