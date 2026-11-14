//! Non-secret user settings: one JSON file in the app config dir, written atomically.
//! The strip's `local_key` never goes through here (spec §12).

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::ops::RangeInclusive;
use std::path::Path;

use light_core::detect::DetectorParams;
use serde::{Deserialize, Serialize};

const MAX_NAME: usize = 63;
/// A real file is under 1 KiB; a bigger one is hand-edited garbage and must not be read whole.
const MAX_BYTES: u64 = 64 << 10;

/// `serde_json`'s own message echoes the offending value; only its kind and position are safe to log.
fn describe(e: &serde_json::Error) -> String {
    format!("{:?} at {}:{}", e.classify(), e.line(), e.column())
}

/// Reads at most `max` bytes: `None` when the file is bigger.
pub(crate) fn read_capped(path: &Path, max: u64) -> io::Result<Option<String>> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(max + 1)
        .read_to_string(&mut text)?;
    Ok((text.len() as u64 <= max).then_some(text))
}

/// When the Dock icon shows; anything else is rejected by serde.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShowInDock {
    /// Only while the editor window is open.
    #[default]
    Editor,
    Always,
}

/// Whether the app is a regular (Dock) app right now.
#[must_use]
pub const fn dock_visible(mode: ShowInDock, editor_open: bool) -> bool {
    matches!(mode, ShowInDock::Always) || editor_open
}

/// Most audio inputs whose detector settings are remembered besides the active one.
pub const MAX_INPUTS: usize = 16;
/// Key of "System default" in `detector_by_input` (never a valid device name: it is empty).
pub const SYSTEM_DEFAULT: &str = "";

/// The detector settings that belong to one audio input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectorConfig {
    pub sensitivity: u8,
    pub calm_kick_db: u8,
    pub calm_high_db: u8,
    pub riser_db: u8,
}

impl Default for DetectorConfig {
    /// The "Reset to defaults" values.
    fn default() -> Self {
        Self {
            sensitivity: 5,
            calm_kick_db: 16,
            calm_high_db: 12,
            riser_db: 7,
        }
    }
}

impl DetectorConfig {
    fn validate(self) -> Result<(), String> {
        if !(1..=10).contains(&self.sensitivity) {
            return Err("sensitivity must be 1 to 10".into());
        }
        if !CALM_KICK_DB.contains(&self.calm_kick_db) {
            return Err("kick threshold must be 8 to 30 dB".into());
        }
        if !CALM_HIGH_DB.contains(&self.calm_high_db) {
            return Err("highs threshold must be 6 to 24 dB".into());
        }
        if !RISER_DB.contains(&self.riser_db) {
            return Err("riser threshold must be 3 to 15 dB".into());
        }
        Ok(())
    }
}

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
    /// Detection panel: whole dB under the learned level that count as quiet. 8..=30.
    pub calm_kick_db: u8,
    /// 6..=24.
    pub calm_high_db: u8,
    /// Rise of the bar means of highs that makes a riser. 3..=15.
    pub riser_db: u8,
    /// Show the `?` help tooltips in the editor.
    pub help_tooltips: bool,
    pub show_in_dock: ShowInDock,
    /// Detector settings of the inputs that are not active (the active input's live in the
    /// top-level fields above). Owned by Rust: `switch_input` rewrites it on every save.
    pub detector_by_input: BTreeMap<String, DetectorConfig>,
}

pub const CALM_KICK_DB: RangeInclusive<u8> = 8..=30;
pub const CALM_HIGH_DB: RangeInclusive<u8> = 6..=24;
pub const RISER_DB: RangeInclusive<u8> = 3..=15;

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
            calm_kick_db: 16,
            calm_high_db: 12,
            riser_db: 7,
            help_tooltips: true,
            show_in_dock: ShowInDock::Editor,
            detector_by_input: BTreeMap::new(),
        }
    }
}

pub(crate) fn check_name(what: &str, name: Option<&String>) -> Result<(), String> {
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
        self.detector().validate()?;
        if self.detector_by_input.len() > MAX_INPUTS {
            return Err("too many audio inputs".into());
        }
        for (name, cfg) in &self.detector_by_input {
            if !name.is_empty() {
                check_name("audio input", Some(name))?;
            }
            cfg.validate()?;
        }
        if !(10..=100).contains(&self.brightness_pct) {
            return Err("brightness must be 10 to 100".into());
        }
        if self.lead_ms > 500 {
            return Err("lead must be 0 to 500 ms".into());
        }
        Ok(())
    }

    /// The active input's detector settings.
    #[must_use]
    pub const fn detector(&self) -> DetectorConfig {
        DetectorConfig {
            sensitivity: self.sensitivity,
            calm_kick_db: self.calm_kick_db,
            calm_high_db: self.calm_high_db,
            riser_db: self.riser_db,
        }
    }

    fn input_key(input: Option<&String>) -> String {
        input.cloned().unwrap_or_else(|| SYSTEM_DEFAULT.to_string())
    }

    /// Call on a settings value about to replace `prev`. The map always comes from `prev` (the
    /// UI never edits it). If the audio input changed, the outgoing input's values are kept and
    /// the incoming input's come back, or the defaults if it is new. Old files need no migration:
    /// their top-level values are the current input's.
    pub fn switch_input(&mut self, prev: &Settings) {
        self.detector_by_input = prev.detector_by_input.clone();
        if self.audio_input == prev.audio_input {
            return;
        }
        let map = &mut self.detector_by_input;
        map.insert(Self::input_key(prev.audio_input.as_ref()), prev.detector());
        let cfg = map
            .remove(&Self::input_key(self.audio_input.as_ref()))
            .unwrap_or_default();
        // Never evict System default or the entry just stored.
        let keep = Self::input_key(prev.audio_input.as_ref());
        while map.len() > MAX_INPUTS {
            let Some(k) = map
                .keys()
                .find(|k| **k != keep && *k != SYSTEM_DEFAULT)
                .cloned()
            else {
                break;
            };
            map.remove(&k);
        }
        (
            self.sensitivity,
            self.calm_kick_db,
            self.calm_high_db,
            self.riser_db,
        ) = (
            cfg.sensitivity,
            cfg.calm_kick_db,
            cfg.calm_high_db,
            cfg.riser_db,
        );
    }
}

/// Defaults on a missing, corrupt or invalid file.
#[must_use]
pub fn load(path: &Path) -> Settings {
    let text = match read_capped(path, MAX_BYTES) {
        Ok(Some(t)) => t,
        Ok(None) => {
            log::warn!("settings too large, using defaults");
            keep_aside(path, &path.with_extension("json.bad"));
            return Settings::default();
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                log::warn!("settings unreadable, using defaults: {e}");
                keep_aside(path, &path.with_extension("json.bad"));
            }
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(s) => match s.validate() {
            Ok(()) => s,
            Err(e) => {
                log::warn!("settings invalid, using defaults: {e}");
                keep_aside(path, &path.with_extension("json.bad"));
                Settings::default()
            }
        },
        Err(e) => {
            log::warn!("settings corrupt, using defaults: {}", describe(&e));
            keep_aside(path, &path.with_extension("json.bad"));
            Settings::default()
        }
    }
}

/// Spec §12: an invalid file is copied to `<name>.bad` so the next save never loses the user's data.
pub(crate) fn keep_aside(path: &Path, bad: &Path) {
    if let Err(e) = std::fs::copy(path, bad) {
        log::warn!("cannot keep a copy of the invalid settings: {e}");
    }
}

/// Writes to a temp file next to `path`, then renames over it.
///
/// # Errors
/// Returns the I/O (or serialisation) error.
pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(&tmp, path)
}

/// Detector thresholds: the panel sets the base, the popover's sensitivity shifts the calm ones
/// (higher means Calm sooner: they drop `sensitivity - 5` dB). Riser is not shifted.
#[must_use]
pub fn detector_params(s: &Settings) -> DetectorParams {
    let shift = f32::from(s.sensitivity) - 5.0;
    DetectorParams {
        calm_kick_db: f32::from(s.calm_kick_db) - shift,
        calm_high_db: f32::from(s.calm_high_db) - shift,
        riser_db: f32::from(s.riser_db),
        ..DetectorParams::default()
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
    fn settings_without_help_tooltips_defaults_true() {
        let (_d, p) = tmp();
        std::fs::write(&p, r#"{"phraseBars":4}"#).unwrap();
        let s = load(&p);
        assert!(s.help_tooltips);
        assert_eq!(s.phrase_bars, 4);
        assert!(Settings::default().help_tooltips);
    }

    #[test]
    fn show_in_dock_defaults_to_editor_and_old_files_load() {
        let (_d, p) = tmp();
        std::fs::write(&p, r#"{"phraseBars":4}"#).unwrap();
        assert_eq!(load(&p).show_in_dock, ShowInDock::Editor);
        std::fs::write(&p, r#"{"showInDock":"always"}"#).unwrap();
        assert_eq!(load(&p).show_in_dock, ShowInDock::Always);
        assert_eq!(
            serde_json::to_string(&ShowInDock::Editor).unwrap(),
            r#""editor""#
        );
    }

    #[test]
    fn unknown_show_in_dock_value_is_rejected() {
        assert!(serde_json::from_str::<Settings>(r#"{"showInDock":"never"}"#).is_err());
    }

    #[test]
    fn dock_is_visible_when_always_or_while_the_editor_is_open() {
        assert!(dock_visible(ShowInDock::Always, false));
        assert!(dock_visible(ShowInDock::Always, true));
        assert!(dock_visible(ShowInDock::Editor, true));
        assert!(!dock_visible(ShowInDock::Editor, false));
    }

    #[test]
    fn oversized_file_is_kept_aside_and_defaults_load() {
        let (d, p) = tmp();
        let body = format!(r#"{{"phraseBars":4}}{}"#, " ".repeat(65 * 1024));
        std::fs::write(&p, body).unwrap();
        assert_eq!(load(&p), Settings::default());
        assert!(d.path().join("settings.json.bad").exists());
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
    fn non_utf8_file_is_kept_aside_before_defaults() {
        let (d, p) = tmp();
        std::fs::write(&p, [0xff, 0xfe, 0x00, 0x80]).unwrap();
        assert_eq!(load(&p), Settings::default());
        assert_eq!(
            std::fs::read(d.path().join("settings.json.bad")).unwrap(),
            [0xff, 0xfe, 0x00, 0x80]
        );
    }

    #[test]
    fn eviction_keeps_system_default_and_the_key_just_stored() {
        // "" and "zz" sort around the evictable keys; both must survive a full map.
        let mut cur = on(None);
        for i in 0..MAX_INPUTS + 3 {
            let next = Settings {
                audio_input: Some(format!("in{i:02}")),
                ..cur.clone()
            };
            cur = saved(&cur, next);
        }
        let last = Settings {
            audio_input: Some("zz".into()),
            ..cur.clone()
        };
        cur = saved(&cur, last);
        let back = Settings {
            audio_input: Some("in00".into()),
            ..cur.clone()
        };
        cur = saved(&cur, back);
        assert!(cur.detector_by_input.contains_key(SYSTEM_DEFAULT));
        assert!(cur.detector_by_input.contains_key("zz"));
        assert!(cur.detector_by_input.len() <= MAX_INPUTS);
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
    fn save_creates_missing_config_dir() {
        let (d, _) = tmp();
        let p = d.path().join("fresh").join("config").join("settings.json");
        save(&p, &Settings::default()).unwrap();
        assert_eq!(load(&p), Settings::default());
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

    fn on(input: Option<&str>) -> Settings {
        Settings {
            audio_input: input.map(String::from),
            ..Settings::default()
        }
    }

    /// What the app does on a save: the UI's value, then the switch against the stored one.
    fn saved(prev: &Settings, mut new: Settings) -> Settings {
        new.switch_input(prev);
        new
    }

    #[test]
    fn old_file_keeps_its_values_for_the_current_input() {
        let old: Settings =
            serde_json::from_str(r#"{"audioInput":"USB","sensitivity":8,"calmKickDb":20}"#)
                .unwrap();
        assert!(old.detector_by_input.is_empty());
        let away = saved(
            &old,
            Settings {
                audio_input: None,
                ..old.clone()
            },
        );
        assert_eq!(
            away.detector(),
            DetectorConfig::default(),
            "new input: defaults"
        );
        let back = saved(
            &away,
            Settings {
                audio_input: Some("USB".into()),
                ..away.clone()
            },
        );
        assert_eq!((back.sensitivity, back.calm_kick_db), (8, 20));
    }

    #[test]
    fn each_input_keeps_its_thresholds() {
        let a = Settings {
            sensitivity: 9,
            riser_db: 4,
            ..on(Some("Mic A"))
        };
        let b = saved(&a, on(Some("Mic B")));
        assert_eq!(b.detector(), DetectorConfig::default());
        let b = Settings {
            calm_high_db: 20,
            ..b
        };
        let a2 = saved(
            &b,
            Settings {
                audio_input: Some("Mic A".into()),
                ..b.clone()
            },
        );
        assert_eq!((a2.sensitivity, a2.riser_db, a2.calm_high_db), (9, 4, 12));
        let b2 = saved(
            &a2,
            Settings {
                audio_input: Some("Mic B".into()),
                ..a2.clone()
            },
        );
        assert_eq!((b2.sensitivity, b2.calm_high_db), (5, 20));
        let sys = saved(
            &b2,
            Settings {
                audio_input: None,
                ..b2.clone()
            },
        );
        assert!(!sys.detector_by_input.contains_key(SYSTEM_DEFAULT));
        let back = saved(
            &sys,
            Settings {
                audio_input: Some("Mic B".into()),
                ..sys.clone()
            },
        );
        assert!(back.detector_by_input.contains_key(SYSTEM_DEFAULT));
        assert!(back.validate().is_ok());
    }

    #[test]
    fn the_ui_cannot_edit_the_map_and_it_stays_bounded() {
        let mut cur = on(Some("in0"));
        let mut forged = cur.clone();
        forged.detector_by_input.insert(
            "x".into(),
            DetectorConfig {
                sensitivity: 9,
                ..Default::default()
            },
        );
        assert!(saved(&cur, forged).detector_by_input.is_empty());
        for i in 1..40 {
            let next = Settings {
                audio_input: Some(format!("in{i}")),
                ..cur.clone()
            };
            cur = saved(&cur, next);
            assert!(cur.detector_by_input.len() <= MAX_INPUTS);
            assert!(cur.validate().is_ok());
        }
    }

    #[test]
    fn the_map_is_validated() {
        let with = |k: &str, c: DetectorConfig| {
            let mut s = Settings::default();
            s.detector_by_input.insert(k.into(), c);
            s
        };
        let ok = DetectorConfig::default();
        assert!(with("USB", ok).validate().is_ok());
        assert!(with(SYSTEM_DEFAULT, ok).validate().is_ok());
        assert!(with("bad\nname", ok).validate().is_err());
        assert!(with(" x", ok).validate().is_err());
        assert!(
            with("USB", DetectorConfig { riser_db: 99, ..ok })
                .validate()
                .is_err()
        );
        let mut many = Settings::default();
        for i in 0..=MAX_INPUTS {
            many.detector_by_input.insert(format!("in{i}"), ok);
        }
        assert!(many.validate().is_err());
    }

    #[test]
    fn thresholds_default_to_the_detector_and_old_files_load() {
        let d = DetectorParams::default();
        let s = Settings::default();
        assert_eq!(
            (
                f32::from(s.calm_kick_db),
                f32::from(s.calm_high_db),
                f32::from(s.riser_db)
            ),
            (d.calm_kick_db, d.calm_high_db, d.riser_db)
        );
        let old: Settings = serde_json::from_str(r#"{"sensitivity":7}"#).unwrap();
        assert_eq!((old.sensitivity, old.calm_kick_db), (7, 16));
        let bad = |f: fn(&mut Settings)| {
            let mut s = Settings::default();
            f(&mut s);
            s.validate().is_err()
        };
        assert!(bad(|s| s.calm_kick_db = 7) && bad(|s| s.calm_kick_db = 31));
        assert!(bad(|s| s.calm_high_db = 5) && bad(|s| s.calm_high_db = 25));
        assert!(bad(|s| s.riser_db = 2) && bad(|s| s.riser_db = 16));
        assert!(!bad(|s| s.riser_db = 15));
    }

    #[test]
    fn sensitivity_5_is_default_params() {
        let d = DetectorParams::default();
        let st = |sensitivity| Settings {
            sensitivity,
            ..Settings::default()
        };
        let p = detector_params(&st(5));
        assert!((p.riser_db - d.riser_db).abs() < 1e-6);
        assert!((p.calm_kick_db - d.calm_kick_db).abs() < 1e-6);
        assert!((p.calm_high_db - d.calm_high_db).abs() < 1e-6);
        assert!(detector_params(&st(10)).calm_kick_db < d.calm_kick_db);
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        fn any_valid() -> impl Strategy<Value = Settings> {
            (
                proptest::option::of("[a-zA-Z0-9][a-zA-Z0-9 ]{0,60}[a-zA-Z0-9]"),
                prop::sample::select(vec![4u8, 8, 16]),
                1u8..=10,
                10u8..=100,
                0u16..=500,
                any::<bool>(),
                (8u8..=30, 6u8..=24, 3u8..=15),
            )
                .prop_map(
                    |(
                        m,
                        phrase_bars,
                        sensitivity,
                        brightness_pct,
                        lead_ms,
                        lights_on,
                        (k, h, r),
                    )| {
                        Settings {
                            midi_source: m,
                            phrase_bars,
                            sensitivity,
                            brightness_pct,
                            lead_ms,
                            lights_on,
                            calm_kick_db: k,
                            calm_high_db: h,
                            riser_db: r,
                            ..Settings::default()
                        }
                    },
                )
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            #[test]
            fn arbitrary_bytes_never_panic_and_never_yield_invalid_settings(
                raw in proptest::collection::vec(any::<u8>(), 0..2048)
            ) {
                let (_d, p) = tmp();
                std::fs::write(&p, &raw).unwrap();
                let s = load(&p);
                prop_assert!(s.validate().is_ok());
                // Garbage is never accepted: unless the bytes are a valid file, defaults come back.
                if serde_json::from_slice::<Settings>(&raw).map_or(true, |x| x.validate().is_err()) {
                    prop_assert_eq!(s, Settings::default());
                }
            }

            #[test]
            fn valid_settings_round_trip(s in any_valid()) {
                prop_assert!(s.validate().is_ok());
                let (_d, p) = tmp();
                save(&p, &s).unwrap();
                prop_assert_eq!(load(&p), s);
            }
        }
    }
}
