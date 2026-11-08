//! Deeboo Lighting: menu bar app shell and IPC.

use tauri::Manager;

mod commands;
mod devices;
#[cfg(debug_assertions)]
mod lan;
#[cfg(debug_assertions)]
mod mic_probe;
#[cfg(debug_assertions)]
mod midi_probe;
mod settings;
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
        commands::get_settings,
        commands::save_settings,
        commands::list_midi_sources,
        commands::list_audio_inputs,
        commands::ui_command,
        commands::dev_paint_test,
        commands::dev_mic_level
    ]);
    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::get_status,
        commands::get_settings,
        commands::save_settings,
        commands::list_midi_sources,
        commands::list_audio_inputs,
        commands::ui_command
    ]);
    builder
        .setup(|app| {
            tray::setup(app)?;
            app.state::<commands::AppState>()
                .init_settings(app.path().app_config_dir()?.join("settings.json"));
            #[cfg(debug_assertions)]
            {
                // The dev command's permission exists only in debug manifests (build.rs).
                app.add_capability(include_str!("../capabilities-dev/popover-dev.json"))?;
                strip::start_dev_connection(app.handle());
                start_midi_probe(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
}

/// Debug only: MIDI inputs (virtual + existing sources) that prove `CoreMIDI` works in the sandbox.
#[cfg(debug_assertions)]
fn start_midi_probe(app: &tauri::AppHandle) {
    let handle = app.clone();
    match midi_probe::start("Deeboo Lighting In", move |n| {
        commands::publish_clock_ticks(&handle, n);
    }) {
        Ok(conns) => {
            app.state::<commands::AppState>().set_midi(conns);
            commands::publish_clock_ticks(app, 0);
        }
        Err(e) => log::warn!("MIDI probe not started: {e}"),
    }
}
