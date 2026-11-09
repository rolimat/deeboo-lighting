//! The strip file (`strip.json`): holds the local key, so it is 0600, atomic and strictly validated.

use std::fmt;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::net::Ipv4Addr;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub struct StripConfig {
    pub id: String,
    pub name: String,
    pub local_key: SecretString,
    /// Wired segments (N): the ones painted.
    pub segments: u8,
    /// What the controller reports in DP 61 (M >= N): the header always carries it.
    pub reported: u8,
    /// Hint only: callers re-check it against the local subnets before use.
    pub last_ip: Option<Ipv4Addr>,
}

impl fmt::Debug for StripConfig {
    /// Names the strip only: no key, no address.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StripConfig")
            .field("name", &self.name)
            .field("segments", &self.segments)
            .field("reported", &self.reported)
            .finish_non_exhaustive()
    }
}

impl StripConfig {
    /// The one rule set for a strip: the store, the file loader and the wizard all use it.
    ///
    /// # Errors
    /// `Invalid` names the first bad field (never its value).
    pub fn validate(&self) -> Result<(), StoreError> {
        if !valid_id(&self.id) {
            return Err(StoreError::Invalid("id"));
        }
        if self.name.chars().count() > 63 || self.name.chars().any(char::is_control) {
            return Err(StoreError::Invalid("name"));
        }
        if self.local_key.expose_secret().len() != 16 {
            return Err(StoreError::Invalid("key"));
        }
        if !(1..=60).contains(&self.reported) {
            return Err(StoreError::Invalid("reported"));
        }
        if !(1..=self.reported).contains(&self.segments) {
            return Err(StoreError::Invalid("segments"));
        }
        Ok(())
    }
}

/// A device id: 1..=64 ASCII alphanumerics.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Never carries file contents or the key.
#[derive(Debug)]
pub enum StoreError {
    Io,
    Mode,
    Invalid(&'static str),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io => f.write_str("strip file cannot be read"),
            Self::Mode => f.write_str("strip file mode must be 0600"),
            Self::Invalid(field) => write!(f, "strip file has an invalid {field}"),
        }
    }
}

const fn one() -> u8 {
    1
}

const fn default_segments() -> u8 {
    10
}

/// `version` and `last_ip` are optional so the Plan 3 file still loads.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StripFile {
    #[serde(default = "one")]
    version: u8,
    id: String,
    name: String,
    local_key: Zeroizing<String>,
    #[serde(default = "default_segments")]
    segments: u8,
    /// Old files: every segment is wired.
    #[serde(default)]
    reported: Option<u8>,
    #[serde(default)]
    last_ip: Option<Ipv4Addr>,
}

#[derive(Serialize)]
struct StripFileOut<'a> {
    version: u8,
    id: &'a str,
    name: &'a str,
    local_key: &'a str,
    segments: u8,
    reported: u8,
    last_ip: Option<Ipv4Addr>,
}

/// # Errors
/// Fails if the file is unreadable, not mode 0600, or any field is invalid.
pub fn load(path: &Path) -> Result<StripConfig, StoreError> {
    let mode = fs::metadata(path)
        .map_err(|_| StoreError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if mode != 0o600 {
        return Err(StoreError::Mode);
    }
    // The raw bytes hold the key: wiped when dropped, like the parsed copy.
    let raw = Zeroizing::new(fs::read(path).map_err(|_| StoreError::Io)?);
    let f: StripFile = serde_json::from_slice(&raw).map_err(|_| StoreError::Invalid("format"))?;
    if f.version != 1 {
        return Err(StoreError::Invalid("version"));
    }
    let cfg = StripConfig {
        id: f.id,
        name: f.name,
        local_key: SecretString::from(f.local_key.as_str()),
        segments: f.segments,
        reported: f.reported.unwrap_or(f.segments),
        last_ip: f.last_ip,
    };
    cfg.validate()?;
    Ok(cfg)
}

/// Room for any valid config (name <= 63 chars x 4 bytes, id 64, key 16, ~150 of JSON), so the buffer
/// never reallocates and leaves an unwiped copy of the key behind.
const SERIALIZED_CAP: usize = 4096;

/// Borrowed fields, wiped buffer: no other owned copy of the key is made here.
fn serialize(cfg: &StripConfig) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(SERIALIZED_CAP));
    serde_json::to_writer(
        &mut *bytes,
        &StripFileOut {
            version: 1,
            id: &cfg.id,
            name: &cfg.name,
            local_key: cfg.local_key.expose_secret(),
            segments: cfg.segments,
            reported: cfg.reported,
            last_ip: cfg.last_ip,
        },
    )
    .map_err(io::Error::other)?;
    debug_assert_eq!(bytes.capacity(), SERIALIZED_CAP);
    Ok(bytes)
}

/// Atomic: temp file created 0600, written, fsynced, then renamed over `path`.
///
/// # Errors
/// Any I/O error; `path` needs a parent directory and a file name.
pub fn save(path: &Path, cfg: &StripConfig) -> io::Result<()> {
    cfg.validate()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    let bad = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "path needs parent and file name",
        )
    };
    let dir = path.parent().ok_or_else(bad)?;
    let name = path.file_name().ok_or_else(bad)?;
    let bytes = serialize(cfg)?;
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let mut tmp_name = name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = dir.join(tmp_name);
    let _ = fs::remove_file(&tmp);
    let written = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef";

    fn mode(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    fn put(dir: &Path, body: &str, m: u32) -> std::path::PathBuf {
        let p = dir.join("strip.json");
        fs::write(&p, body).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(m)).unwrap();
        p
    }

    #[test]
    fn maximal_config_serialises_without_reallocating() {
        let cfg = StripConfig {
            id: "a".repeat(64),
            name: "\u{1F4A1}".repeat(63),
            local_key: SecretString::from(KEY),
            segments: 60,
            reported: 60,
            last_ip: Some(Ipv4Addr::new(192, 168, 100, 200)),
        };
        cfg.validate().unwrap();
        let bytes = serialize(&cfg).unwrap();
        assert_eq!(bytes.capacity(), SERIALIZED_CAP);
        let q = StripConfig {
            name: "\"".repeat(63),
            ..cfg
        };
        assert_eq!(serialize(&q).unwrap().capacity(), SERIALIZED_CAP);
    }

    #[test]
    fn save_then_load_round_trips_and_is_0600() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub").join("strip.json");
        let cfg = StripConfig {
            id: "bf12AB".into(),
            name: "dj lights".into(),
            local_key: SecretString::from(KEY),
            segments: 24,
            reported: 24,
            last_ip: Some(Ipv4Addr::new(192, 168, 1, 7)),
        };
        save(&p, &cfg).unwrap();
        assert_eq!(mode(&p), 0o600);
        let back = load(&p).unwrap();
        assert_eq!(
            (
                back.id.as_str(),
                back.name.as_str(),
                back.segments,
                back.last_ip
            ),
            ("bf12AB", "dj lights", 24, cfg.last_ip)
        );
        assert_eq!(back.local_key.expose_secret(), KEY);
        assert!(!d.path().join("sub").join("strip.json.tmp").exists());
        // Overwriting keeps 0600.
        save(&p, &cfg).unwrap();
        assert_eq!(mode(&p), 0o600);
    }

    #[test]
    fn load_refuses_group_readable_and_invalid() {
        let d = tempfile::tempdir().unwrap();
        let ok = |extra: &str| {
            format!(r#"{{"version":1,"id":"abc","name":"n","local_key":"{KEY}"{extra}}}"#)
        };
        assert!(load(&put(d.path(), &ok(""), 0o600)).is_ok());
        assert!(matches!(
            load(&put(d.path(), &ok(""), 0o644)),
            Err(StoreError::Mode)
        ));
        assert!(matches!(
            load(&put(d.path(), &ok(""), 0o640)),
            Err(StoreError::Mode)
        ));
        let long_name = "x".repeat(64);
        let bad = [
            ok(r#","id":"a b""#),
            ok(r#","id":"""#),
            ok(&format!(r#","id":"{}""#, "a".repeat(65))),
            ok(&format!(r#","name":"{long_name}""#)),
            ok(r#","name":"a\nb""#),
            ok(r#","local_key":"short""#),
            ok(r#","segments":0"#),
            ok(r#","segments":61"#),
            ok(r#","version":2"#),
            ok(r#","last_ip":"not an ip""#),
            ok(r#","extra":1"#),
            "{".into(),
            String::new(),
        ];
        for body in &bad {
            assert!(
                load(&put(d.path(), body, 0o600)).is_err(),
                "accepted: {body}"
            );
        }
        assert!(matches!(
            load(&d.path().join("missing.json")),
            Err(StoreError::Io)
        ));
    }

    #[test]
    fn save_refuses_what_load_would_refuse() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("strip.json");
        let good = || StripConfig {
            id: "bf12AB".into(),
            name: "dj".into(),
            local_key: SecretString::from(KEY),
            segments: 4,
            reported: 4,
            last_ip: None,
        };
        let mut short_key = good();
        short_key.local_key = SecretString::from(&KEY[..15]);
        let mut bad_id = good();
        bad_id.id = "a b".into();
        let mut no_segments = good();
        no_segments.segments = 0;
        for cfg in [short_key, bad_id, no_segments] {
            assert!(cfg.validate().is_err());
            assert!(save(&p, &cfg).is_err());
            assert!(!p.exists(), "nothing is written for an invalid strip");
        }
        assert!(good().validate().is_ok());
    }

    #[test]
    fn reads_plan3_format() {
        let d = tempfile::tempdir().unwrap();
        let body = format!(r#"{{"id":"abc","name":"dj lights","local_key":"{KEY}"}}"#);
        let cfg = load(&put(d.path(), &body, 0o600)).unwrap();
        assert_eq!((cfg.segments, cfg.last_ip), (10, None));
    }

    #[test]
    fn old_strip_file_reported_defaults_to_segments() {
        let d = tempfile::tempdir().unwrap();
        let body = format!(r#"{{"id":"abc","name":"n","local_key":"{KEY}","segments":8}}"#);
        let cfg = load(&put(d.path(), &body, 0o600)).unwrap();
        assert_eq!((cfg.segments, cfg.reported), (8, 8));
    }

    #[test]
    fn wired_above_reported_rejected() {
        let d = tempfile::tempdir().unwrap();
        let body =
            format!(r#"{{"id":"abc","name":"n","local_key":"{KEY}","segments":9,"reported":8}}"#);
        assert!(matches!(
            load(&put(d.path(), &body, 0o600)),
            Err(StoreError::Invalid("segments"))
        ));
    }

    #[test]
    fn wired_and_reported_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("strip.json");
        let cfg = StripConfig {
            id: "abc".into(),
            name: "n".into(),
            local_key: SecretString::from(KEY),
            segments: 8,
            reported: 10,
            last_ip: None,
        };
        save(&p, &cfg).unwrap();
        let back = load(&p).unwrap();
        assert_eq!((back.segments, back.reported), (8, 10));
        assert_eq!(mode(&p), 0o600);
    }

    #[test]
    fn error_display_has_no_key() {
        let d = tempfile::tempdir().unwrap();
        let body = format!(r#"{{"id":"a b","name":"n","local_key":"{KEY}"}}"#);
        let e = load(&put(d.path(), &body, 0o600)).err().unwrap();
        for s in [e.to_string(), format!("{e:?}")] {
            assert!(
                !s.is_empty() && !s.contains(KEY) && !s.contains("a b"),
                "{s}"
            );
        }
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            #[test]
            fn arbitrary_bytes_never_panic_and_are_refused(
                raw in proptest::collection::vec(any::<u8>(), 0..2048)
            ) {
                let d = tempfile::tempdir().unwrap();
                let p = d.path().join("strip.json");
                fs::write(&p, &raw).unwrap();
                fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
                prop_assert!(load(&p).is_err());
            }

            #[test]
            fn valid_strips_round_trip(
                id in "[a-zA-Z0-9]{1,64}",
                name in "[ -~]{0,63}",
                key in "[ -~]{16}",
                segments in 1u8..=60,
                ip in proptest::option::of(any::<[u8; 4]>().prop_map(Ipv4Addr::from)),
            ) {
                let d = tempfile::tempdir().unwrap();
                let p = d.path().join("strip.json");
                let cfg = StripConfig {
                    id: id.clone(),
                    name: name.clone(),
                    local_key: SecretString::from(key.clone()),
                    segments,
                    reported: segments,
                    last_ip: ip,
                };
                save(&p, &cfg).unwrap();
                let back = load(&p).unwrap();
                prop_assert_eq!(
                    (back.id, back.name, back.local_key.expose_secret().to_owned(), back.segments, back.last_ip),
                    (id, name, key, segments, ip)
                );
            }
        }
    }
}
