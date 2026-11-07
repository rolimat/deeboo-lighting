use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Writes `contents` atomically (temp file `0600`, then rename); the parent is created `0700`.
///
/// # Errors
/// Any I/O error; `path` needs a parent directory and a file name.
pub fn write_secret_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let bad = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "path needs parent and file name",
        )
    };
    let dir = path.parent().ok_or_else(bad)?;
    let name = path.file_name().ok_or_else(bad)?;
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let mut tmp_name = name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = dir.join(tmp_name);
    let _ = fs::remove_file(&tmp);
    let written = (|| {
        let mut f: File = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, path)?;
    // The rename replaces any old file, but re-apply in case umask/ACLs differ.
    fs::set_permissions(path, Permissions::from_mode(0o600))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn secret_file_is_owner_only() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(".dev").join("strip.json");
        write_secret_file(&p, b"one").unwrap();
        assert_eq!(mode(&p), 0o600);
        assert_eq!(mode(p.parent().unwrap()), 0o700);
        fs::set_permissions(&p, Permissions::from_mode(0o644)).unwrap();
        write_secret_file(&p, b"two").unwrap();
        assert_eq!(mode(&p), 0o600);
        assert_eq!(fs::read(&p).unwrap(), b"two");
    }
}
