//! Deeboo Lighting: menu bar app shell and IPC.

use tauri::{Manager, RunEvent};

mod audio;
mod clock;
mod commands;
mod devices;
mod lan;
mod midi;
mod qr;
mod runtime;
mod settings;
mod strip;
mod strip_store;
mod tray;
#[cfg_attr(not(test), expect(dead_code, reason = "wired to IPC in Plan 4 task 6"))]
mod wizard;

/// Env-driven logger; the network stacks stay at `warn` even with `RUST_LOG=trace`, so
/// cloud request details can never reach the log.
fn logger() -> env_logger::Builder {
    let mut b = env_logger::Builder::from_default_env();
    b.filter_module("ureq", log::LevelFilter::Warn)
        .filter_module("rustls", log::LevelFilter::Warn);
    b
}

/// Starts the tray app.
///
/// # Errors
/// Returns the Tauri error if the runtime fails to start.
pub fn run() -> tauri::Result<()> {
    logger().init();
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::save_settings,
            commands::list_midi_sources,
            commands::list_audio_inputs,
            commands::ui_command,
            commands::quit
        ])
        .setup(|app| {
            tray::setup(app)?;
            let dir = app.path().app_config_dir()?;
            let state = app.state::<commands::AppState>();
            state.init_settings(dir.join("settings.json"));
            let handle = runtime::spawn(app.handle().clone(), state.settings(), strip::load(&dir));
            state.set_runtime(handle);
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<commands::AppState>().quit();
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use log::{Level, Record};

    fn allowed(target: &str, level: Level) -> bool {
        let log = super::logger().build();
        log.matches(&Record::builder().target(target).level(level).build())
    }

    #[test]
    fn log_filter_caps_ureq_and_rustls() {
        for t in [
            "ureq",
            "ureq::unversioned::transport",
            "rustls",
            "rustls::client",
        ] {
            assert!(!allowed(t, Level::Info), "{t} info");
            assert!(!allowed(t, Level::Debug), "{t} debug");
            assert!(allowed(t, Level::Warn), "{t} warn");
        }
    }
}
