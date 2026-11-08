//! The strip file (`strip.json`): holds the local key, so it is 0600, atomic and strictly validated.

use std::fmt;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::net::Ipv4Addr;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

pub struct StripConfig {
    pub id: String,
    pub name: String,
    pub local_key: SecretString,
    pub segments: u8,
    /// Hint only: callers re-check it against the local subnets before use.
    pub last_ip: Option<Ipv4Addr>,
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
    local_key: String,
    #[serde(default = "default_segments")]
    segments: u8,
    #[serde(default)]
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
    let f: StripFile = serde_json::from_slice(&fs::read(path).map_err(|_| StoreError::Io)?)
        .map_err(|_| StoreError::Invalid("format"))?;
    if f.version != 1 {
        return Err(StoreError::Invalid("version"));
    }
    if !(1..=64).contains(&f.id.len()) || !f.id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(StoreError::Invalid("id"));
    }
    if f.name.chars().count() > 63 || f.name.chars().any(char::is_control) {
        return Err(StoreError::Invalid("name"));
    }
    if f.local_key.len() != 16 {
        return Err(StoreError::Invalid("key"));
    }
    if !(1..=60).contains(&f.segments) {
        return Err(StoreError::Invalid("segments"));
    }
    Ok(StripConfig {
        id: f.id,
        name: f.name,
        local_key: SecretString::from(f.local_key),
        segments: f.segments,
        last_ip: f.last_ip,
    })
}

/// Atomic: temp file created 0600, written, fsynced, then renamed over `path`.
///
/// # Errors
/// Any I/O error; `path` needs a parent directory and a file name.
pub fn save(path: &Path, cfg: &StripConfig) -> io::Result<()> {
    let bad = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "path needs parent and file name",
        )
    };
    let dir = path.parent().ok_or_else(bad)?;
    let name = path.file_name().ok_or_else(bad)?;
    let json = serde_json::json!({
        "version": 1,
        "id": cfg.id,
        "name": cfg.name,
        "local_key": cfg.local_key.expose_secret(),
        "segments": cfg.segments,
        "last_ip": cfg.last_ip,
    });
    let bytes = serde_json::to_vec(&json).map_err(io::Error::other)?;
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
    fn save_then_load_round_trips_and_is_0600() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub").join("strip.json");
        let cfg = StripConfig {
            id: "bf12AB".into(),
            name: "dj lights".into(),
            local_key: SecretString::from(KEY),
            segments: 24,
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
    fn reads_plan3_format() {
        let d = tempfile::tempdir().unwrap();
        let body = format!(r#"{{"id":"abc","name":"dj lights","local_key":"{KEY}"}}"#);
        let cfg = load(&put(d.path(), &body, 0o600)).unwrap();
        assert_eq!((cfg.segments, cfg.last_ip), (10, None));
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
}
