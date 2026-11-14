//! Learned detector references per audio input (`detector-refs.json`): not secret, so normal
//! permissions. Same rules as settings: capped read, invalid files kept aside, atomic write.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use light_core::detect::{LearnedRefs, REF_DB_RANGE};
use serde::{Deserialize, Serialize};

use crate::settings::{MAX_INPUTS, SYSTEM_DEFAULT, check_name, keep_aside, read_capped};

pub const FILE: &str = "detector-refs.json";
pub const VERSION: u16 = 1;
const MAX_BYTES: u64 = 64 << 10;
const MAX_RING: usize = 64;

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefsFile {
    pub version: u16,
    pub inputs: BTreeMap<String, Entry>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub written: u64,
    pub refs: LearnedRefs,
}

impl RefsFile {
    /// # Errors
    /// Returns what is out of range.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err("unknown version".into());
        }
        if self.inputs.len() > MAX_INPUTS {
            return Err("too many inputs".into());
        }
        for (key, e) in &self.inputs {
            if key != SYSTEM_DEFAULT {
                check_name("audio input", Some(key))?;
            }
            for ring in [&e.refs.kick, &e.refs.sub, &e.refs.high] {
                if ring.len() > MAX_RING || ring.iter().any(|v| !REF_DB_RANGE.contains(v)) {
                    return Err("reference out of range".into());
                }
            }
        }
        Ok(())
    }

    /// Stores `refs` for `key`; over `MAX_INPUTS` the oldest other input (never the system
    /// default) goes.
    pub fn put(&mut self, key: &str, refs: LearnedRefs) {
        self.version = VERSION;
        let written = self.inputs.values().map(|e| e.written).max().unwrap_or(0) + 1;
        self.inputs.insert(key.to_string(), Entry { written, refs });
        while self.inputs.len() > MAX_INPUTS {
            let Some(old) = self
                .inputs
                .iter()
                .filter(|(k, _)| *k != key && *k != SYSTEM_DEFAULT)
                .min_by_key(|(_, e)| e.written)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.inputs.remove(&old);
        }
    }
}

/// Missing file: empty. An unreadable, oversize, corrupt or invalid one is copied to `.bad`
/// first, so the next save never loses it.
#[must_use]
pub fn load(path: &Path) -> RefsFile {
    let bad = path.with_extension("json.bad");
    let text = match read_capped(path, MAX_BYTES) {
        Ok(Some(t)) => t,
        Ok(None) => {
            log::warn!("detector references too large, ignored");
            keep_aside(path, &bad);
            return RefsFile::default();
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                log::warn!("detector references unreadable, ignored: {e}");
                keep_aside(path, &bad);
            }
            return RefsFile::default();
        }
    };
    match serde_json::from_str::<RefsFile>(&text) {
        Ok(f) => match f.validate() {
            Ok(()) => return f,
            Err(e) => log::warn!("detector references invalid, ignored: {e}"),
        },
        Err(e) => log::warn!(
            "detector references corrupt, ignored: {:?} at {}:{}",
            e.classify(),
            e.line(),
            e.column()
        ),
    }
    keep_aside(path, &bad);
    RefsFile::default()
}

/// Writes to a temp file next to `path`, then renames over it.
///
/// # Errors
/// Returns the I/O (or serialisation) error.
pub fn save(path: &Path, f: &RefsFile) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(f)?)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(v: f32) -> LearnedRefs {
        LearnedRefs {
            kick: vec![v; 3],
            sub: vec![v],
            high: vec![],
        }
    }

    fn file_with(key: &str) -> RefsFile {
        let mut f = RefsFile {
            version: VERSION,
            ..RefsFile::default()
        };
        f.put(key, refs(-20.0));
        f
    }

    fn tmp() -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(FILE);
        (d, p)
    }

    #[test]
    fn round_trip() {
        let (_d, p) = tmp();
        let f = file_with("Mic A");
        save(&p, &f).unwrap();
        assert_eq!(load(&p), f);
    }

    #[test]
    fn missing_file_loads_empty() {
        let (_d, p) = tmp();
        assert!(load(&p).inputs.is_empty());
    }

    #[test]
    fn corrupt_file_is_kept_aside_and_empty_loads() {
        let (_d, p) = tmp();
        std::fs::write(&p, b"{garbage").unwrap();
        assert!(load(&p).inputs.is_empty());
        assert_eq!(
            std::fs::read(p.with_extension("json.bad")).unwrap(),
            b"{garbage"
        );
    }

    #[test]
    fn oversize_and_invalid_values_are_kept_aside() {
        let good = serde_json::to_string(&file_with("Mic")).unwrap();
        let cases = [
            "x".repeat(70 << 10),
            good.replace("-20.0", "\"NaN\""),
            good.replace(
                "\"sub\":[-20.0]",
                &format!("\"sub\":[{}]", vec!["-20.0"; 65].join(",")),
            ),
            good.replace("\"version\":1", "\"version\":2"),
            good.replace("Mic", "a\\u0007b"),
            good.replace("-20.0", "99.0"),
        ];
        for bad in cases {
            let (_d, p) = tmp();
            std::fs::write(&p, &bad).unwrap();
            assert!(load(&p).inputs.is_empty(), "{}", &bad[..bad.len().min(80)]);
            assert!(p.with_extension("json.bad").exists());
        }
    }

    #[test]
    fn put_evicts_oldest_written_but_never_system_default() {
        let mut f = RefsFile {
            version: VERSION,
            ..RefsFile::default()
        };
        f.put(SYSTEM_DEFAULT, refs(-1.0));
        for i in 1..=MAX_INPUTS {
            f.put(&format!("in{i}"), refs(-2.0));
        }
        assert_eq!(f.inputs.len(), MAX_INPUTS);
        assert!(f.inputs.contains_key(SYSTEM_DEFAULT));
        assert!(f.inputs.contains_key(&format!("in{MAX_INPUTS}")));
        assert!(!f.inputs.contains_key("in1"));
        assert!(f.validate().is_ok());
    }
}
