//! Deeboo Lighting: menu bar app shell and IPC.

#[cfg(debug_assertions)]
use tauri::Manager;

mod commands;
#[cfg(debug_assertions)]
mod lan;
#[cfg(debug_assertions)]
mod strip;
mod tray;

/// Starts the tray app.
///
/// # Errors
/// Returns the Tauri error if the runtime fails to start.
pub fn run() -> tauri::Result<()> {
    env_logger::init();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(commands::AppState::default());
    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::get_status,
        commands::dev_paint_test
    ]);
    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![commands::get_status]);
    builder
        .setup(|app| {
            tray::setup(app)?;
            #[cfg(debug_assertions)]
            {
                // The dev command's permission exists only in debug manifests (build.rs).
                app.add_capability(include_str!("../capabilities-dev/popover-dev.json"))?;
                strip::start_dev_connection(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
}
