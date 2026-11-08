//! IPC commands. Every DTO here is mirrored in `ui/src/api.ts`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TrySendError;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use light_core::detect::Moment;
use light_core::engine::UiCommand;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::runtime::{Input, RuntimeHandle};

use crate::library_store;
use crate::settings::{self, Settings};
use crate::wizard::session::{Emit, RealLan, Wizard};
use light_core::library::Library;
use light_core::model::{Sequence, resolve};
use light_core::scene;

/// The most the app waits at exit for the wizard to revoke its cloud session.
const QUIT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StripStatus {
    NotConfigured,
    Connecting,
    Connected {
        name: String,
        segments: u8,
    },
    Error,
    /// The strip refused the saved key (re-added in Smart Life): pair again.
    KeyInvalid,
    /// Discovery did not see the strip's ID.
    NotFound,
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

/// The look the popover names: the active set and the palette in force.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub set: String,
    pub palette: PaletteView,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteView {
    pub name: String,
    /// Principal, Contraste, Suave.
    pub colours: [light_core::paint::Hsv; 3],
}

impl Selection {
    pub fn of(lib: &Library) -> Self {
        let p = lib.current_palette();
        Self {
            set: lib
                .current_set()
                .map_or_else(String::new, |s| s.name.clone()),
            palette: PaletteView {
                name: p.name,
                colours: [p.principal, p.contraste, p.suave],
            },
        }
    }
}

/// Which list `select` steps through.
#[derive(Clone, Copy)]
pub enum Pick {
    Set,
    Palette,
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
    pub selection: Selection,
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
            selection: Selection::of(&Library::builtin(10)),
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
    library: Mutex<Library>,
    /// `library.json`; set at startup.
    library_path: Mutex<Option<std::path::PathBuf>>,
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
            library: Mutex::new(Library::builtin(10)),
            library_path: Mutex::new(None),
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

    /// Loads the library file (built-ins for `segments` on any problem) and remembers its path.
    pub fn init_library(&self, path: std::path::PathBuf, segments: u8) {
        *lock(&self.library) = library_store::load(&path, segments);
        *lock(&self.library_path) = Some(path);
    }

    pub fn library(&self) -> Library {
        lock(&self.library).clone()
    }

    /// Steps the active set or palette by one, wrapping, and saves it like an editor save.
    pub fn select(&self, pick: Pick, delta: i8) -> Result<Selection, String> {
        if delta != 1 && delta != -1 {
            return Err("delta must be -1 or 1".into());
        }
        let mut lib = self.library();
        let step = |ids: Vec<u32>, cur: Option<u32>| -> Option<u32> {
            let at = cur
                .and_then(|c| ids.iter().position(|i| *i == c))
                .unwrap_or(0);
            let len = ids.len().max(1);
            let next = if delta > 0 {
                (at + 1) % len
            } else {
                (at + len - 1) % len
            };
            ids.get(next).copied()
        };
        match pick {
            Pick::Set => {
                let ids = lib.sets.iter().map(|s| s.id).collect();
                lib.active_set = step(ids, Some(lib.active_set)).ok_or("no sets")?;
            }
            Pick::Palette => {
                let ids = lib.palettes.iter().map(|p| p.0).collect();
                lib.active_palette = step(ids, lib.current_palette_id());
            }
        }
        self.save_library(lib).map(|l| Selection::of(&l))
    }

    /// Validates, assigns ids to new entries, persists, then swaps. Any failure keeps the previous
    /// library on disk and in memory; messages never echo the input.
    fn save_library(&self, mut new: Library) -> Result<Library, String> {
        let path = lock(&self.library_path)
            .clone()
            .ok_or("library unavailable")?;
        new.validate().map_err(|e| e.to_string())?;
        let mut current = lock(&self.library);
        library_store::assign_ids(&current, &mut new).map_err(str::to_string)?;
        library_store::save(&path, &new).map_err(|e| {
            log::warn!("library not saved: {e}");
            "library not saved".to_string()
        })?;
        *current = new.clone();
        Ok(new)
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

    /// Tells the engine to pause or resume its strip connection. A pause that got lost would leave
    /// the strip offline, so a full queue hands it to a short-lived thread, like `report_strip`.
    fn send_strip_pause(&self, pause: bool) {
        let Some(tx) = lock(&self.runtime).as_ref().map(|h| h.tx.clone()) else {
            return;
        };
        if let Err(TrySendError::Full(input)) = tx.try_send(Input::StripPause(pause)) {
            let _ = std::thread::Builder::new()
                .name("strip-pause".into())
                .spawn(move || {
                    let _ = tx.send(input);
                });
        }
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

    /// App exit: a running wizard is cancelled (its cloud session revoked) before the engine stops.
    pub fn quit(&self, emit: &Emit) {
        self.wizard.shutdown(emit, QUIT_WAIT);
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
#[allow(clippy::needless_pass_by_value)] // tauri::State is passed by value by the macro
pub fn get_library(state: tauri::State<'_, AppState>) -> Library {
    state.library()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn select_set(state: tauri::State<'_, AppState>, delta: i8) -> Result<Selection, String> {
    select(&state, Pick::Set, delta)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn select_palette(state: tauri::State<'_, AppState>, delta: i8) -> Result<Selection, String> {
    select(&state, Pick::Palette, delta)
}

fn select(state: &AppState, pick: Pick, delta: i8) -> Result<Selection, String> {
    let sel = state.select(pick, delta)?;
    state.send(Input::Library(Box::new(state.library())))?;
    Ok(sel)
}

/// Entries whose id the stored library lacks are new; the saved library (with real ids) is returned.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn save_library(
    state: tauri::State<'_, AppState>,
    library: Library,
) -> Result<Library, String> {
    let saved = state.save_library(library)?;
    state.send(Input::Library(Box::new(saved.clone())))?;
    Ok(saved)
}

/// Messages each step costs to reach from the previous one (the first from the last: it wraps),
/// so the editor can flag steps the strip would show in several parts. Messages never echo input.
fn message_counts_of(lib: &Library, seq: &Sequence, palette_id: u32) -> Result<Vec<u8>, String> {
    if seq.steps.len() > light_core::library::MAX_STEPS {
        return Err("A sequence has at most 64 steps.".into());
    }
    seq.validate(lib.segments())
        .map_err(|_| "Every step needs one role per segment.".to_string())?;
    let (_, palette) = lib
        .palettes
        .iter()
        .find(|(id, _)| *id == palette_id)
        .ok_or("No such palette.")?;
    let scenes: Vec<Vec<_>> = seq
        .steps
        .iter()
        .map(|s| s.roles.iter().map(|r| resolve(*r, palette, 100)).collect())
        .collect();
    let n = scenes.len();
    Ok((0..n)
        .map(|i| {
            let count = scene::diff(&scenes[(i + n - 1) % n], &scenes[i]).len();
            u8::try_from(count).unwrap_or(u8::MAX)
        })
        .collect())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn message_counts(
    state: tauri::State<'_, AppState>,
    sequence: Sequence,
    palette_id: u32,
) -> Result<Vec<u8>, String> {
    message_counts_of(&state.library(), &sequence, palette_id)
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
    let handle = app.clone();
    let pause: Arc<dyn Fn(bool) + Send + Sync> =
        Arc::new(move |p| handle.state::<AppState>().send_strip_pause(p));
    state.wizard.start(
        Arc::new(RealLan(pause)),
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

/// The strip must be connected and `segment` (1-based) one of its segments.
fn identify_check(strip: &StripStatus, segment: u8) -> Result<(), String> {
    match strip {
        StripStatus::Connected { segments, .. } if (1..=*segments).contains(&segment) => Ok(()),
        StripStatus::Connected { .. } => Err("No such segment.".into()),
        _ => Err("Strip not connected.".into()),
    }
}

/// Flashes one segment through the app's own strip connection (the shape editor, after pairing).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn identify_segment(state: tauri::State<'_, AppState>, segment: u8) -> Result<(), String> {
    identify_check(&lock(&state.status).strip, segment)?;
    state.send(Input::Identify(segment))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub fn wizard_save(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    // `Saved` goes out only once the engine has the new strip; if it cannot take the news now, the
    // file is safe on disk and a restart still picks it up.
    state
        .wizard
        .save(&state.strip_path()?, &wizard_emit(&app), |cfg| {
            state.send(Input::StripChanged(cfg)).map_err(|e| {
                log::warn!("strip saved but not reloaded: {e}");
                "Saved. Restart the app to use the new strip.".to_string()
            })
        })
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
            r#"{"strip":{"kind":"notConfigured"},"bpm":null,"playing":false,"beatInBar":null,"barInPhrase":null,"phraseBars":8,"moment":"rhythm","auto":true,"predicted":false,"lightsOn":true,"midi":{"source":null,"ok":true},"audio":{"ok":false},"selection":{"set":"Default","palette":{"name":"Test","colours":[{"h":0,"s":1000,"v":1000},{"h":240,"s":1000,"v":1000},{"h":120,"s":350,"v":1000}]}}}"#
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
        assert_eq!(commands.len(), 20);
        assert_eq!(granted, commands);
    }

    #[test]
    fn identify_check_needs_a_connected_strip_and_a_segment_in_range() {
        let on = StripStatus::Connected {
            name: "dj".into(),
            segments: 10,
        };
        assert_eq!(identify_check(&on, 1), Ok(()));
        assert_eq!(identify_check(&on, 10), Ok(()));
        for bad in [0, 11] {
            assert_eq!(identify_check(&on, bad), Err("No such segment.".into()));
        }
        assert_eq!(
            identify_check(&StripStatus::Connecting, 1),
            Err("Strip not connected.".into())
        );
    }

    #[test]
    fn message_counts_matches_scene_diff() {
        use Role::{Contraste as C, Off, Principal as P};
        use light_core::model::{Role, Step, StepLen};
        let lib = Library::builtin(4);
        let step = |roles: [Role; 4]| Step {
            len: StepLen::One,
            roles: roles.to_vec(),
        };
        let seq = Sequence {
            name: "t".into(),
            moment: Moment::Rhythm,
            steps: vec![
                step([P, P, P, P]),         // from the last (all off): 1 bucket
                step([C, P, Off, Off]),     // a pencil and an eraser: 2
                step([Off, Off, Off, Off]), // 1 eraser
                step([Off, Off, Off, Off]), // unchanged: 0
            ],
        };
        assert_eq!(message_counts_of(&lib, &seq, 1), Ok(vec![1, 2, 1, 0]));
        assert!(message_counts_of(&lib, &seq, 99).is_err());
        let mut short = seq.clone();
        short.steps[1].roles.pop();
        assert!(message_counts_of(&lib, &short, 1).is_err());
        short.steps.clear();
        assert!(message_counts_of(&lib, &short, 1).is_err());
        let mut long = seq.clone();
        long.steps = vec![seq.steps[0].clone(); light_core::library::MAX_STEPS + 1];
        assert!(
            message_counts_of(&lib, &long, 1).is_err(),
            "IPC input is capped"
        );
    }

    fn lib_state() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::default();
        state.init_library(dir.path().join("library.json"), 10);
        (dir, state)
    }

    #[test]
    fn save_library_rejects_invalid_and_keeps_previous() {
        let (dir, state) = lib_state();
        let before = state.library();
        let mut bad = before.clone();
        bad.sets[0].rhythm.palettes = vec![4242];
        let msg = state.save_library(bad).unwrap_err();
        assert!(msg.contains("palettes") && !msg.contains("4242"), "{msg}");
        assert_eq!(state.library(), before);
        assert!(!dir.path().join("library.json").exists());
    }

    #[test]
    fn palette_cycles_and_persists() {
        let (dir, state) = lib_state();
        let mut lib = state.library();
        let mut pal = lib.palettes[0].1.clone();
        pal.name = "Second".into();
        lib.palettes.push((900, pal));
        state.save_library(lib).unwrap();
        let name = |sel: Selection| sel.palette.name;
        assert_eq!(name(state.select(Pick::Palette, 1).unwrap()), "Second");
        assert_eq!(state.library().active_palette, Some(2));
        // Wraps both ways.
        assert_eq!(name(state.select(Pick::Palette, 1).unwrap()), "Test");
        assert_eq!(name(state.select(Pick::Palette, -1).unwrap()), "Second");
        assert_eq!(
            crate::library_store::load(&dir.path().join("library.json"), 10).active_palette,
            Some(2)
        );
        for bad in [0, 2, -2] {
            assert!(state.select(Pick::Palette, bad).is_err());
            assert!(state.select(Pick::Set, bad).is_err());
        }
    }

    #[test]
    fn set_cycles_through_sets() {
        let (_dir, state) = lib_state();
        let mut lib = state.library();
        let mut set = lib.sets[0].clone();
        set.name = "Club".into();
        set.id = 900;
        lib.sets.push(set);
        state.save_library(lib).unwrap();
        assert_eq!(state.select(Pick::Set, 1).unwrap().set, "Club");
        assert_eq!(state.select(Pick::Set, 1).unwrap().set, "Default");
        assert_eq!(state.select(Pick::Set, -1).unwrap().set, "Club");
    }

    #[test]
    fn new_entries_get_ids_in_rust() {
        use light_core::library::SeqRef;
        let (dir, state) = lib_state();
        let mut lib = state.library();
        // Ids the stored library does not have are new: the client's number is only a handle.
        let pal = lib.palettes[0].1.clone();
        let seq = lib.sequences[0].1.clone();
        lib.palettes.push((900, pal));
        lib.sequences.push((901, seq));
        let mut set = lib.sets[0].clone();
        set.id = 902;
        set.rhythm.sequences = vec![SeqRef::Id(901)];
        set.rhythm.palettes = vec![900];
        lib.sets.push(set);
        lib.active_set = 902;
        lib.active_palette = Some(900);

        let saved = state.save_library(lib).unwrap();
        assert_eq!(
            saved.palettes.iter().map(|p| p.0).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(
            saved.sequences.iter().map(|p| p.0).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(saved.sets[1].id, 2);
        assert_eq!(saved.sets[1].rhythm.sequences, [SeqRef::Id(4)]);
        assert_eq!(saved.sets[1].rhythm.palettes, [2]);
        assert_eq!((saved.active_set, saved.active_palette), (2, Some(2)));
        assert_eq!(state.library(), saved);
        assert_eq!(
            crate::library_store::load(&dir.path().join("library.json"), 10),
            saved
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
