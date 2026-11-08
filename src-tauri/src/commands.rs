//! IPC commands. Every DTO here is mirrored in `ui/src/api.ts`.

#[cfg(debug_assertions)]
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, PoisonError};

use light_core::detect::Moment;
use light_core::engine::UiCommand;
use serde::{Deserialize, Serialize};

use crate::settings::{self, Settings};

#[cfg(debug_assertions)]
use crate::strip::Strip;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StripStatus {
    NotConfigured,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    Connecting,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    Connected {
        name: String,
        segments: u8,
    },
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub strip: StripStatus,
    /// True in debug builds only: the UI shows dev tools then.
    pub debug: bool,
    /// MIDI clock ticks seen by the debug probe; `None` in release or before it is up.
    #[serde(rename = "clockTicks")]
    pub clock_ticks: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MomentDto {
    Rhythm,
    Calm,
    BuildUp,
}

/// What the popover can ask of the show; anything else is rejected by serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiCommandDto {
    Lights { on: bool },
    Force { moment: MomentDto },
    Auto,
}

impl From<UiCommandDto> for UiCommand {
    fn from(c: UiCommandDto) -> Self {
        match c {
            UiCommandDto::Lights { on } => UiCommand::Lights(on),
            UiCommandDto::Force { moment } => UiCommand::Force(match moment {
                MomentDto::Rhythm => Moment::Rhythm,
                MomentDto::Calm => Moment::Calm,
                MomentDto::BuildUp => Moment::BuildUp,
            }),
            UiCommandDto::Auto => UiCommand::Auto,
        }
    }
}

/// Mutable app state shared with the commands.
pub struct AppState {
    status: Mutex<StripStatus>,
    clock_ticks: Mutex<Option<u64>>,
    settings: Mutex<Settings>,
    /// Where the settings file lives; set at startup.
    settings_path: Mutex<Option<std::path::PathBuf>>,
    #[cfg(debug_assertions)]
    strip: Mutex<Option<Arc<Strip>>>,
    /// Keeps the debug MIDI connections alive for the app's lifetime.
    #[cfg(debug_assertions)]
    midi: Mutex<Vec<midir::MidiInputConnection<()>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: Mutex::new(StripStatus::NotConfigured),
            clock_ticks: Mutex::new(None),
            settings: Mutex::new(Settings::default()),
            settings_path: Mutex::new(None),
            #[cfg(debug_assertions)]
            strip: Mutex::new(None),
            #[cfg(debug_assertions)]
            midi: Mutex::new(Vec::new()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    #[cfg(debug_assertions)]
    fn set_strip_status(&self, s: StripStatus) {
        *lock(&self.status) = s;
    }

    #[cfg(debug_assertions)]
    pub fn set_strip(&self, strip: Arc<Strip>) {
        *lock(&self.strip) = Some(strip);
    }

    #[cfg(debug_assertions)]
    pub fn set_midi(&self, conns: Vec<midir::MidiInputConnection<()>>) {
        *lock(&self.midi) = conns;
    }

    /// Loads the settings file (defaults on any problem) and remembers its path.
    pub fn init_settings(&self, path: std::path::PathBuf) {
        *lock(&self.settings) = settings::load(&path);
        *lock(&self.settings_path) = Some(path);
    }

    fn save_settings(&self, new: Settings) -> Result<Settings, String> {
        new.validate()?;
        let path = lock(&self.settings_path)
            .clone()
            .ok_or("settings unavailable")?;
        settings::save(&path, &new).map_err(|e| {
            log::warn!("settings not saved: {e}");
            "settings not saved".to_string()
        })?;
        *lock(&self.settings) = new.clone();
        Ok(new)
    }

    fn status(&self) -> Status {
        Status {
            strip: lock(&self.status).clone(),
            debug: cfg!(debug_assertions),
            clock_ticks: *lock(&self.clock_ticks),
        }
    }
}

/// Event the popover listens to; its payload is the whole [`Status`].
#[cfg(debug_assertions)]
const STATUS_EVENT: &str = "status";

/// Stores the strip status and pushes the new [`Status`] to the popover.
#[cfg(debug_assertions)]
pub fn publish_strip_status(app: &tauri::AppHandle, s: StripStatus) {
    use tauri::Manager;
    app.state::<AppState>().set_strip_status(s);
    emit_status(app);
}

/// Stores the debug MIDI clock count and pushes the new [`Status`] to the popover.
#[cfg(debug_assertions)]
pub fn publish_clock_ticks(app: &tauri::AppHandle, n: u64) {
    use tauri::Manager;
    *lock(&app.state::<AppState>().clock_ticks) = Some(n);
    emit_status(app);
}

#[cfg(debug_assertions)]
fn emit_status(app: &tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    let state = app.state::<AppState>();
    if let Err(e) = app.emit_to("popover", STATUS_EVENT, state.status()) {
        log::warn!("status event not delivered: {e}");
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_status(state: tauri::State<'_, AppState>) -> Status {
    state.status()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    lock(&state.settings).clone()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn save_settings(
    state: tauri::State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    state.save_settings(settings)
}

#[tauri::command]
pub async fn list_midi_sources() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(crate::devices::midi_sources)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_audio_inputs() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(crate::devices::audio_inputs)
        .await
        .map_err(|e| e.to_string())
}

/// Validated by serde on the way in. The engine consumes it in the live wiring (Plan 3, Task 9);
/// until then only the lights switch is applied (and persisted), the rest is accepted and dropped.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn ui_command(state: tauri::State<'_, AppState>, cmd: UiCommandDto) -> Result<(), String> {
    if let UiCommandDto::Lights { on } = cmd {
        let mut s = lock(&state.settings).clone();
        s.lights_on = on;
        state.save_settings(s)?;
    }
    Ok(())
}

/// Debug builds only; absent from the release handler list, manifest and capabilities.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn dev_paint_test(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let StripStatus::Connected { segments, .. } = lock(&state.status).clone() else {
        return Err("strip not connected".into());
    };
    let strip = lock(&state.strip).clone().ok_or("strip not connected")?;
    strip.paint_test(segments).await.map_err(|e| e.to_string())
}

/// Debug builds only: peak level of the default input over 1 s, in dBFS.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn dev_mic_level() -> Result<f32, String> {
    tauri::async_runtime::spawn_blocking(crate::mic_probe::peak_dbfs)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serialises_as_the_ui_expects() {
        let json = |s: &Status| serde_json::to_string(s).unwrap_or_default();
        let state = AppState::default();
        assert_eq!(
            json(&state.status()),
            format!(
                r#"{{"strip":{{"kind":"notConfigured"}},"debug":{},"clockTicks":null}}"#,
                cfg!(debug_assertions)
            )
        );
        let connected = Status {
            strip: StripStatus::Connected {
                name: "dj lights".into(),
                segments: 10,
            },
            debug: false,
            clock_ticks: Some(48),
        };
        assert_eq!(
            json(&connected),
            r#"{"strip":{"kind":"connected","name":"dj lights","segments":10},"debug":false,"clockTicks":48}"#
        );
    }

    #[test]
    fn ui_command_dto_parses_and_rejects_unknown() {
        let parse = |j: &str| serde_json::from_str::<UiCommandDto>(j);
        assert_eq!(
            parse(r#"{"kind":"lights","on":false}"#).ok(),
            Some(UiCommandDto::Lights { on: false })
        );
        assert_eq!(
            UiCommand::from(parse(r#"{"kind":"force","moment":"buildUp"}"#).unwrap()),
            UiCommand::Force(Moment::BuildUp)
        );
        assert_eq!(parse(r#"{"kind":"auto"}"#).ok(), Some(UiCommandDto::Auto));
        assert!(parse(r#"{"kind":"reboot"}"#).is_err());
        assert!(parse(r#"{"kind":"force","moment":"loud"}"#).is_err());
    }
}
