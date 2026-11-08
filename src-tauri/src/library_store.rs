//! The user library (`library.json`): not secret, so normal permissions. Atomic writes with five
//! rotated backups; an invalid, oversized or unreadable file is kept as `.bad` and the built-ins load.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use std::collections::HashMap;

use light_core::library::{Library, SeqRef};

/// A hand-edited multi-GB file must not blow memory: larger files count as invalid.
/// The largest library `validate` accepts is ~2.3 MB compact.
const MAX_BYTES: u64 = 4 << 20;
/// Kept-aside invalid files: `.bad`, `.bad.1` .. `.bad.4`.
const BAD_KEPT: u32 = 5;
const BACKUPS: u32 = 5;

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn read_capped(path: &Path) -> io::Result<Option<String>> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_string(&mut text)?;
    Ok((text.len() as u64 <= MAX_BYTES).then_some(text))
}

/// `serde_json`'s own message echoes the offending value; only its kind and position are safe to log.
fn describe(e: &serde_json::Error) -> String {
    format!("{:?} at {}:{}", e.classify(), e.line(), e.column())
}

/// The first free `.bad`, `.bad.1`, ...; when all are taken, the oldest one (it is overwritten).
fn bad_slot(path: &Path) -> PathBuf {
    let slots: Vec<PathBuf> = (0..BAD_KEPT)
        .map(|i| {
            sibling(
                path,
                &if i == 0 {
                    ".bad".into()
                } else {
                    format!(".bad.{i}")
                },
            )
        })
        .collect();
    let age = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    slots
        .iter()
        .find(|p| !p.exists())
        .or_else(|| slots.iter().min_by_key(|p| age(p)))
        .cloned()
        .unwrap_or_else(|| sibling(path, ".bad"))
}

/// Built-ins for `segments` on a missing, corrupt or invalid file. A valid library built for a
/// different segment count is kept as is (the engine falls back, see plan 5).
#[must_use]
pub fn load(path: &Path, segments: u8) -> Library {
    let parsed = match read_capped(path) {
        Ok(Some(t)) => serde_json::from_str::<Library>(&t)
            .map_err(|e| describe(&e))
            .and_then(|l| l.validate().map(|()| l).map_err(|e| e.to_string())),
        Ok(None) => Err("file too large".to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Library::builtin(segments),
        Err(e) => Err(e.to_string()),
    };
    parsed.unwrap_or_else(|e| {
        log::warn!("library invalid, using built-ins: {e}");
        // Moved, not copied: a huge file is not duplicated, and the next save cannot overwrite it.
        if let Err(e) = std::fs::rename(path, bad_slot(path)) {
            log::warn!("cannot keep the invalid library aside: {e}");
        }
        Library::builtin(segments)
    })
}

/// Rotates `.1`..`.5` (the oldest is dropped), then writes tmp + fsync + rename.
///
/// # Errors
/// Returns the I/O (or serialisation) error; the previous file stays in place.
pub fn save(path: &Path, lib: &Library) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = sibling(path, ".tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(&serde_json::to_vec(lib)?)?;
    f.sync_all()?;
    drop(f);
    if path.exists() {
        for i in (1..BACKUPS).rev() {
            let from = sibling(path, &format!(".{i}"));
            if from.exists() {
                std::fs::rename(&from, sibling(path, &format!(".{}", i + 1)))?;
            }
        }
        // Copy, not rename: `path` stays valid if the final rename fails.
        std::fs::copy(path, sibling(path, ".1"))?;
    }
    std::fs::rename(&tmp, path)
}

/// Ids are Rust's: an id the stored library does not have is a client-side handle for a new entry.
/// Each such id becomes the next free one (per kind, in order of appearance), and every reference
/// to it (set contents, active set/palette) is remapped consistently. `new` must be validated.
///
/// # Errors
/// When the id space is exhausted.
pub fn assign_ids(prev: &Library, new: &mut Library) -> Result<(), &'static str> {
    fn remap<T>(
        known: &[u32],
        items: &mut [T],
        id: impl Fn(&mut T) -> &mut u32,
    ) -> Result<HashMap<u32, u32>, &'static str> {
        let mut next = known.iter().max().map_or(Some(1), |m| m.checked_add(1));
        let mut map = HashMap::new();
        for it in items {
            let id = id(it);
            if !known.contains(id) {
                let n = next.ok_or("too many entries")?;
                next = n.checked_add(1);
                map.insert(*id, n);
                *id = n;
            }
        }
        Ok(map)
    }
    let pal = remap(&ids(&prev.palettes), &mut new.palettes, |p| &mut p.0)?;
    let seq = remap(&ids(&prev.sequences), &mut new.sequences, |p| &mut p.0)?;
    let prev_sets: Vec<u32> = prev.sets.iter().map(|s| s.id).collect();
    let set = remap(&prev_sets, &mut new.sets, |s| &mut s.id)?;
    for s in &mut new.sets {
        for m in [&mut s.rhythm, &mut s.calm, &mut s.buildup] {
            for p in &mut m.palettes {
                *p = *pal.get(p).unwrap_or(p);
            }
            for r in &mut m.sequences {
                if let SeqRef::Id(i) = r {
                    *i = *seq.get(i).unwrap_or(i);
                }
            }
        }
    }
    new.active_set = *set.get(&new.active_set).unwrap_or(&new.active_set);
    if let Some(p) = &mut new.active_palette {
        *p = *pal.get(p).unwrap_or(p);
    }
    Ok(())
}

fn ids<T>(v: &[(u32, T)]) -> Vec<u32> {
    v.iter().map(|e| e.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn tmp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("library.json");
        (dir, p)
    }

    fn with(edit: impl FnOnce(&mut Value)) -> String {
        let mut v = serde_json::to_value(Library::builtin(10)).unwrap();
        edit(&mut v);
        v.to_string()
    }

    #[test]
    fn invalid_library_is_kept_aside_and_defaults_load() {
        let steps: Vec<Value> = (0..10_000)
            .map(|_| json!({"len":"one","roles":vec!["principal";10]}))
            .collect();
        let tampered = [
            with(|v| v["sets"][0]["rhythm"]["sequences"][0] = json!({"id": 99})),
            with(|v| v["sequences"][0][1]["steps"][0]["roles"] = json!(["principal"])),
            with(|v| v["sequences"][0][1]["steps"] = json!(steps)),
            with(|v| v["shape"]["pieces"][0]["x"] = json!(1e30)),
            with(|v| v["shape"]["pieces"][0]["x"] = json!(null)),
            with(|v| v["extra"] = json!(1)),
            "not json".to_string(),
            " ".repeat(5 << 20),
        ];
        for text in tampered {
            let (dir, p) = tmp();
            std::fs::write(&p, &text).unwrap();
            assert_eq!(load(&p, 10), Library::builtin(10));
            assert_eq!(
                std::fs::read_to_string(dir.path().join("library.json.bad")).unwrap(),
                text
            );
        }
    }

    #[test]
    fn largest_valid_library_survives_a_save_and_reload() {
        let mut lib = Library::builtin(60);
        let seq = lib.sequences[0].1.clone();
        let mut big = seq.clone();
        big.steps = vec![seq.steps[0].clone(); light_core::library::MAX_STEPS];
        lib.sequences = (1..=64).map(|i| (i, big.clone())).collect();
        lib.sets[0].rhythm.sequences = vec![SeqRef::Id(1)];
        lib.sets[0].calm.sequences = vec![SeqRef::Id(1)];
        lib.sets[0].buildup.sequences = vec![SeqRef::Id(1)];
        assert_eq!(lib.validate(), Ok(()));
        let (_d, p) = tmp();
        save(&p, &lib).unwrap();
        assert!(
            std::fs::metadata(&p).unwrap().len() > 1 << 20,
            "past the old cap"
        );
        assert_eq!(load(&p, 10), lib);
    }

    #[test]
    fn parse_errors_are_described_without_the_value() {
        let e = serde_json::from_str::<Library>(r#"{"schemaVersion":"HUNTER2"}"#).unwrap_err();
        assert!(
            e.to_string().contains("HUNTER2"),
            "premise: serde echoes the value"
        );
        assert!(!describe(&e).contains("HUNTER2"), "{}", describe(&e));
    }

    #[test]
    fn later_invalid_libraries_do_not_overwrite_earlier_ones() {
        let (dir, p) = tmp();
        let bad = |n: u32| {
            std::fs::write(&p, format!("not json {n}")).unwrap();
            let _ = load(&p, 10);
        };
        (0..5).for_each(bad);
        let read = |s: &str| std::fs::read_to_string(dir.path().join(s));
        assert_eq!(read("library.json.bad").unwrap(), "not json 0");
        assert_eq!(read("library.json.bad.1").unwrap(), "not json 1");
        assert_eq!(read("library.json.bad.4").unwrap(), "not json 4");
        (5..8).for_each(bad);
        let kept: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".bad")
            })
            .collect();
        assert_eq!(kept.len(), 5, "capped");
        assert!(!dir.path().join("library.json.bad.5").exists());
    }

    #[test]
    fn missing_loads_builtin_and_valid_other_size_is_kept() {
        let (_d, p) = tmp();
        assert_eq!(load(&p, 4), Library::builtin(4));
        save(&p, &Library::builtin(6)).unwrap();
        assert_eq!(load(&p, 10), Library::builtin(6));
    }

    #[test]
    fn existing_user_library_is_not_overwritten() {
        let (_d, p) = tmp();
        let mut lib = Library::builtin(6);
        lib.palettes.truncate(1);
        lib.active_palette = Some(lib.palettes[0].0);
        for s in &mut lib.sets {
            for m in [&mut s.rhythm, &mut s.calm, &mut s.buildup] {
                m.palettes = vec![lib.palettes[0].0];
            }
        }
        lib.sets[0].name = "Mine".into();
        save(&p, &lib).unwrap();
        assert_eq!(load(&p, 6), lib);
    }

    #[test]
    fn save_rotates_five_backups() {
        let (dir, p) = tmp();
        for n in 1..=8u8 {
            save(&p, &Library::builtin(n)).unwrap();
        }
        assert_eq!(load(&p, 1), Library::builtin(8));
        for i in 1..=5u8 {
            let f = dir.path().join(format!("library.json.{i}"));
            let lib: Library = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
            assert_eq!(lib, Library::builtin(8 - i), "backup {i}");
        }
        assert!(!dir.path().join("library.json.6").exists());
        assert!(!dir.path().join("library.json.tmp").exists());
    }

    #[test]
    fn save_creates_missing_dir() {
        let (dir, _) = tmp();
        let p = dir.path().join("a/b/library.json");
        save(&p, &Library::builtin(3)).unwrap();
        assert_eq!(load(&p, 3), Library::builtin(3));
    }
}
