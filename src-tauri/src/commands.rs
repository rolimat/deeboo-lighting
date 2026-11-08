//! IPC commands. Every DTO here is mirrored in `ui/src/api.ts`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TrySendError;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use light_core::detect::Moment;
use light_core::engine::UiCommand;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::runtime::{Input, RuntimeHandle};
use crate::settings::{self, Settings};
use crate::wizard::session::{Emit, RealLan, Wizard};

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
    wizard: Wizard,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: Mutex::new(Status::initial()),
            settings: Mutex::new(Settings::default()),
            settings_path: Mutex::new(None),
            runtime: Mutex::new(None),
            visible: Arc::new(AtomicBool::new(false)),
            wizard: Wizard::default(),
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

    /// `strip.json` next to the settings file.
    fn strip_path(&self) -> Result<std::path::PathBuf, String> {
        let path = lock(&self.settings_path).clone();
        path.and_then(|p| p.parent().map(|d| d.join("strip.json")))
            .ok_or_else(|| "settings unavailable".to_string())
    }

    /// Editor window closed: revoke and forget whatever the wizard holds.
    pub fn editor_closed(&self, emit: &Emit) {
        self.wizard.cancel(emit);
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

/// Pushes wizard states to the editor window only.
pub fn wizard_emit(app: &tauri::AppHandle) -> Emit {
    let app = app.clone();
    Arc::new(move |state| {
        if let Err(e) = app.emit_to("editor", "wizard", &state) {
            log::debug!("wizard event not delivered: {e}");
        }
    })
}

/// The User Code is validated here (`UserCode::parse`) and never logged or echoed.
#[tauri::command]
pub async fn wizard_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    user_code: String,
) -> Result<(), String> {
    let code = tuya_cloud::UserCode::parse(&user_code).map_err(|_| {
        "Enter the User Code from Smart Life (letters and digits only).".to_string()
    })?;
    state.wizard.start(
        Arc::new(RealLan),
        Arc::new(tuya_cloud::UreqTransport::new()),
        code,
        wizard_emit(&app),
    )
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri deserialises owned arguments
pub fn wizard_choose(state: tauri::State<'_, AppState>, device_id: String) -> Result<(), String> {
    state.wizard.choose(&device_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_retry(app: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    state.wizard.retry(&wizard_emit(&app));
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_cancel(app: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    state.wizard.cancel(&wizard_emit(&app));
}

/// `segment` is 1-based and checked against the strip's segment count.
#[tauri::command]
pub async fn strip_identify(state: tauri::State<'_, AppState>, segment: u8) -> Result<(), String> {
    state.wizard.identify(segment).await
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_save(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.wizard.save(&state.strip_path()?, &wizard_emit(&app))
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
        // The popover and editor capabilities together grant exactly those commands.
        let mut granted: Vec<String> = Vec::new();
        for cap in [
            include_str!("../capabilities/popover.json"),
            include_str!("../capabilities/editor.json"),
        ] {
            let cap: serde_json::Value = serde_json::from_str(cap).unwrap_or_default();
            granted.extend(
                cap["permissions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .filter(|p| p.starts_with("allow-"))
                    .map(String::from),
            );
        }
        granted.sort();
        granted.dedup();
        commands.sort();
        assert_eq!(commands.len(), 14);
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
