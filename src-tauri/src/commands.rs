//! IPC commands. Every DTO here is mirrored in `ui/src/api.ts`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TrySendError;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use light_core::detect::Moment;
use light_core::engine::UiCommand;
use serde::{Deserialize, Serialize};

use crate::runtime::{Input, RuntimeHandle};
use crate::settings::{self, Settings};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StripStatus {
    NotConfigured,
    Connecting,
    Connected { name: String, segments: u8 },
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MidiStatus {
    pub source: Option<String>,
    pub ok: bool,
}

/// Live audio: open only while playing with Auto and Lights on.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Peak of the last beat in dBFS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_db: Option<f32>,
}

/// What the popover shows; the engine thread publishes it only while the popover is visible.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // mirrors the UI's status object
pub struct Status {
    pub strip: StripStatus,
    pub bpm: Option<f32>,
    pub playing: bool,
    pub beat_in_bar: Option<u8>,
    pub bar_in_phrase: Option<u8>,
    pub phrase_bars: u8,
    pub moment: MomentDto,
    pub auto: bool,
    pub predicted: bool,
    pub lights_on: bool,
    pub midi: MidiStatus,
    pub audio: AudioStatus,
}

impl Status {
    /// Before the engine thread has said anything.
    fn initial() -> Self {
        Self {
            strip: StripStatus::NotConfigured,
            bpm: None,
            playing: false,
            beat_in_bar: None,
            bar_in_phrase: None,
            phrase_bars: 8,
            moment: MomentDto::Rhythm,
            auto: true,
            predicted: false,
            lights_on: true,
            midi: MidiStatus {
                source: None,
                ok: true,
            },
            audio: AudioStatus::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MomentDto {
    Rhythm,
    Calm,
    BuildUp,
}

impl From<Moment> for MomentDto {
    fn from(m: Moment) -> Self {
        match m {
            Moment::Rhythm => Self::Rhythm,
            Moment::Calm => Self::Calm,
            Moment::BuildUp => Self::BuildUp,
        }
    }
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
    /// The last status the engine thread published.
    status: Mutex<Status>,
    settings: Mutex<Settings>,
    /// Where the settings file lives; set at startup.
    settings_path: Mutex<Option<std::path::PathBuf>>,
    runtime: Mutex<Option<RuntimeHandle>>,
    /// Read by the engine thread: status goes out only while this is true.
    visible: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: Mutex::new(Status::initial()),
            settings: Mutex::new(Settings::default()),
            settings_path: Mutex::new(None),
            runtime: Mutex::new(None),
            visible: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    /// Loads the settings file (defaults on any problem) and remembers its path.
    pub fn init_settings(&self, path: std::path::PathBuf) {
        *lock(&self.settings) = settings::load(&path);
        *lock(&self.settings_path) = Some(path);
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
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

    pub fn set_status(&self, s: Status) {
        *lock(&self.status) = s;
    }

    pub fn visible_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.visible)
    }

    pub fn set_runtime(&self, handle: RuntimeHandle) {
        *lock(&self.runtime) = Some(handle);
    }

    /// Queues an input for the engine thread; never blocks.
    fn send(&self, input: Input) -> Result<(), String> {
        let guard = lock(&self.runtime);
        let handle = guard.as_ref().ok_or("engine not running")?;
        handle.tx.try_send(input).map_err(|e| match e {
            TrySendError::Full(_) => "engine busy".to_string(),
            TrySendError::Disconnected(_) => "engine stopped".to_string(),
        })
    }

    /// Called by the tray when the popover shows or hides.
    pub fn set_popover_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Relaxed);
        if visible && self.send(Input::PopoverShown).is_err() {
            log::debug!("popover shown but the engine did not get the news");
        }
    }

    pub fn quit(&self) {
        let _ = self.send(Input::Quit);
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_status(state: tauri::State<'_, AppState>) -> Status {
    lock(&state.status).clone()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn save_settings(
    state: tauri::State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let saved = state.save_settings(settings)?;
    state.send(Input::Settings(saved.clone()))?;
    Ok(saved)
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

/// Validated by serde on the way in. The lights switch is also persisted.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn ui_command(state: tauri::State<'_, AppState>, cmd: UiCommandDto) -> Result<(), String> {
    if let UiCommandDto::Lights { on } = cmd {
        let mut s = state.settings();
        s.lights_on = on;
        state.save_settings(s)?;
    }
    state.send(Input::Ui(cmd.into()))
}

/// The settings view's Quit button; `RunEvent::Exit` then stops the engine.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serialises_as_the_ui_expects() {
        let json = serde_json::to_string(&Status::initial()).unwrap_or_default();
        assert_eq!(
            json,
            r#"{"strip":{"kind":"notConfigured"},"bpm":null,"playing":false,"beatInBar":null,"barInPhrase":null,"phraseBars":8,"moment":"rhythm","auto":true,"predicted":false,"lightsOn":true,"midi":{"source":null,"ok":true},"audio":{"ok":false}}"#
        );
        let connected = StripStatus::Connected {
            name: "dj lights".into(),
            segments: 10,
        };
        assert_eq!(
            serde_json::to_string(&connected).unwrap_or_default(),
            r#"{"kind":"connected","name":"dj lights","segments":10}"#
        );
    }

    #[test]
    fn release_manifest_has_no_dev_commands() {
        let build = include_str!("../build.rs");
        assert!(!build.contains("dev_"), "build.rs must list no dev command");
        let list = build
            .split("COMMANDS: &[&str] = &[")
            .nth(1)
            .and_then(|r| r.split("];").next())
            .unwrap_or_default();
        let mut commands: Vec<String> = list
            .split('"')
            .skip(1)
            .step_by(2)
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        // The popover capability grants exactly those commands (plus event listening).
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/popover.json")).unwrap_or_default();
        let mut granted: Vec<String> = cap["permissions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .filter(|p| p.starts_with("allow-"))
            .map(String::from)
            .collect();
        commands.sort();
        granted.sort();
        assert_eq!(commands.len(), 7);
        assert_eq!(granted, commands);
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
