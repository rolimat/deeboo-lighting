//! Non-secret user settings: one JSON file in the app config dir, written atomically.
//! The strip's `local_key` never goes through here (spec §12).

use std::io;
use std::path::Path;

use light_core::detect::DetectorParams;
use serde::{Deserialize, Serialize};

const MAX_NAME: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Settings {
    pub midi_source: Option<String>,
    pub audio_input: Option<String>,
    /// 4, 8 or 16.
    pub phrase_bars: u8,
    /// 1..=10.
    pub sensitivity: u8,
    /// 10..=100.
    pub brightness_pct: u8,
    /// 0..=500.
    pub lead_ms: u16,
    pub lights_on: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            midi_source: None,
            audio_input: None,
            phrase_bars: 8,
            sensitivity: 5,
            brightness_pct: 100,
            lead_ms: 100,
            lights_on: true,
        }
    }
}

fn check_name(what: &str, name: Option<&String>) -> Result<(), String> {
    let Some(n) = name else { return Ok(()) };
    if n.trim() != n
        || n.is_empty()
        || n.chars().count() > MAX_NAME
        || n.chars().any(char::is_control)
    {
        return Err(format!("invalid {what} name"));
    }
    Ok(())
}

impl Settings {
    /// # Errors
    /// Returns what is out of range; every IPC argument goes through this.
    pub fn validate(&self) -> Result<(), String> {
        check_name("MIDI source", self.midi_source.as_ref())?;
        check_name("audio input", self.audio_input.as_ref())?;
        if ![4, 8, 16].contains(&self.phrase_bars) {
            return Err("phrase must be 4, 8 or 16 bars".into());
        }
        if !(1..=10).contains(&self.sensitivity) {
            return Err("sensitivity must be 1 to 10".into());
        }
        if !(10..=100).contains(&self.brightness_pct) {
            return Err("brightness must be 10 to 100".into());
        }
        if self.lead_ms > 500 {
            return Err("lead must be 0 to 500 ms".into());
        }
        Ok(())
    }
}

/// Defaults on a missing, corrupt or invalid file.
#[must_use]
pub fn load(path: &Path) -> Settings {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                log::warn!("settings unreadable, using defaults: {e}");
            }
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(s) => match s.validate() {
            Ok(()) => s,
            Err(e) => {
                log::warn!("settings invalid, using defaults: {e}");
                keep_aside(path);
                Settings::default()
            }
        },
        Err(e) => {
            log::warn!("settings corrupt, using defaults: {e}");
            keep_aside(path);
            Settings::default()
        }
    }
}

/// Spec §12: an invalid file is copied to `<name>.bad` so the next save never loses the user's data.
fn keep_aside(path: &Path) {
    let bad = path.with_extension("json.bad");
    if let Err(e) = std::fs::copy(path, &bad) {
        log::warn!("cannot keep a copy of the invalid settings: {e}");
    }
}

/// Writes to a temp file next to `path`, then renames over it.
///
/// # Errors
/// Returns the I/O (or serialisation) error.
pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(&tmp, path)
}

/// Detector thresholds for a sensitivity: higher means Calm sooner (thresholds drop `s - 5` dB).
#[must_use]
#[allow(dead_code)] // built into the detector by the live wiring (Plan 3, Task 9)
pub fn sensitivity_params(s: u8) -> DetectorParams {
    let d = DetectorParams::default();
    let shift = f32::from(s) - 5.0;
    DetectorParams {
        calm_kick_db: d.calm_kick_db - shift,
        calm_high_db: d.calm_high_db - shift,
        ..d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        (dir, p)
    }

    #[test]
    fn invalid_file_is_kept_aside_before_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        let kept = dir.path().join("settings.json.bad");
        assert_eq!(std::fs::read(&kept).unwrap(), b"{not json");
    }

    #[test]
    fn defaults_on_missing_or_corrupt_file() {
        let (_d, p) = tmp();
        assert_eq!(load(&p), Settings::default());
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(load(&p), Settings::default());
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let (_d, p) = tmp();
        std::fs::write(&p, r#"{"phraseBars":5}"#).unwrap();
        assert_eq!(load(&p), Settings::default());
        std::fs::write(&p, r#"{"sensitivity":0}"#).unwrap();
        assert_eq!(load(&p), Settings::default());
    }

    #[test]
    fn save_is_atomic_and_round_trips() {
        let (d, p) = tmp();
        let s = Settings {
            midi_source: Some("IAC Driver".into()),
            phrase_bars: 16,
            lead_ms: 250,
            lights_on: false,
            ..Settings::default()
        };
        save(&p, &s).unwrap();
        assert_eq!(load(&p), s);
        let names: Vec<_> = std::fs::read_dir(d.path()).unwrap().collect();
        assert_eq!(names.len(), 1, "no temp file left behind");
    }

    #[test]
    fn rejects_control_chars_and_long_names() {
        let with = |n: &str| Settings {
            midi_source: Some(n.into()),
            ..Settings::default()
        };
        assert!(with("ok name").validate().is_ok());
        assert!(with("bad\nname").validate().is_err());
        assert!(with("").validate().is_err());
        assert!(with(" padded").validate().is_err());
        assert!(with(&"a".repeat(64)).validate().is_err());
        assert!(with(&"a".repeat(63)).validate().is_ok());
    }

    #[test]
    fn sensitivity_5_is_default_params() {
        let d = DetectorParams::default();
        let p = sensitivity_params(5);
        assert!((p.calm_kick_db - d.calm_kick_db).abs() < 1e-6);
        assert!((p.calm_high_db - d.calm_high_db).abs() < 1e-6);
        assert!(sensitivity_params(10).calm_kick_db < d.calm_kick_db);
    }
}
