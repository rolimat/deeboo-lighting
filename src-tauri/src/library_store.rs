//! The user library (`library.json`): not secret, so normal permissions. Atomic writes with five
//! rotated backups; an invalid, oversized or unreadable file is kept as `.bad` and the built-ins load.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use std::collections::HashMap;

use light_core::library::{Library, SeqRef};
use light_core::migrate::{LibraryV1, LibraryV2, migrate_v1, migrate_v2};

/// A hand-edited multi-GB file must not blow memory: larger files count as invalid.
/// The largest library `validate` accepts is ~4.7 MB compact (128 steps).
const MAX_BYTES: u64 = 8 << 20;
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

/// The first free `<base>`, `<base>.1`, ...; when all are taken, the oldest one (it is overwritten).
fn slot(path: &Path, base: &str) -> PathBuf {
    let slots: Vec<PathBuf> = (0..BAD_KEPT)
        .map(|i| {
            sibling(
                path,
                &if i == 0 {
                    base.to_string()
                } else {
                    format!("{base}.{i}")
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
        .unwrap_or_else(|| sibling(path, base))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    schema_version: u16,
}

/// A schema-1 or 2 file is migrated (spec §6.5): backed up once as `.v1` / `.v2`, then saved as
/// schema 3. A failed migration is an error like any invalid file; a failed backup or save keeps
/// the file and the migrated library is used in memory.
fn parse(path: &Path, text: &str, phrase_bars: u8) -> Result<Library, String> {
    let migrated = match serde_json::from_str(text).map(|v: Version| v.schema_version) {
        Ok(1) => {
            let v1 = serde_json::from_str::<LibraryV1>(text).map_err(|e| describe(&e))?;
            Some((".v1", migrate_v1(v1, phrase_bars)))
        }
        Ok(2) => {
            let v2 = serde_json::from_str::<LibraryV2>(text).map_err(|e| describe(&e))?;
            Some((".v2", migrate_v2(v2)))
        }
        _ => None,
    };
    if let Some((suffix, lib)) = migrated {
        let lib = lib.map_err(|e| e.to_string())?;
        let backup = sibling(path, suffix);
        // `copy` keeps the file's own permissions, as for every other file of the store.
        let kept = backup.exists() || std::fs::copy(path, &backup).is_ok();
        if !kept || save(path, &lib).is_err() {
            log::warn!("library migrated in memory only: cannot back up or save it");
        }
        return Ok(lib);
    }
    serde_json::from_str::<Library>(text)
        .map_err(|e| describe(&e))
        .and_then(|mut l| {
            // Older files may reference another moment's sequence: keep the original beside it.
            if l.drop_foreign_refs() {
                if std::fs::copy(path, slot(path, ".refs")).is_ok() {
                    log::warn!("library had references to another moment's sequence: dropped");
                } else {
                    log::warn!("library references dropped, but the original could not be copied");
                }
            }
            l.validate().map(|()| l).map_err(|e| e.to_string())
        })
}

/// Built-ins for `segments` on a missing, corrupt or invalid file. A valid library built for a
/// different segment count is kept as is (the engine falls back, see plan 5).
#[must_use]
pub fn load(path: &Path, segments: u8, phrase_bars: u8) -> Library {
    let parsed = match read_capped(path) {
        Ok(Some(t)) => parse(path, &t, phrase_bars),
        Ok(None) => Err("file too large".to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Library::builtin(segments),
        Err(e) => Err(e.to_string()),
    };
    parsed.unwrap_or_else(|e| {
        log::warn!("library invalid, using built-ins: {e}");
        // Moved, not copied: a huge file is not duplicated, and the next save cannot overwrite it.
        if let Err(e) = std::fs::rename(path, slot(path, ".bad")) {
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
        for p in &mut s.palettes {
            *p = *pal.get(p).unwrap_or(p);
        }
        for m in [&mut s.rhythm, &mut s.calm, &mut s.buildup] {
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

/// `Library::builtin` plus ten sequences (ids 1-4 Rhythm, 5-7 Calm, 8-10 Build-up), for tests that
/// need sequences to edit.
#[cfg(test)]
pub(crate) fn seeded_builtin(n: u8) -> Library {
    use light_core::detect::Moment::{BuildUp, Calm, Rhythm};
    use light_core::patterns::{build, for_moment};
    let mut l = Library::builtin(n);
    let picks = [(Rhythm, 4), (Calm, 3), (BuildUp, 3)];
    let seqs = picks
        .iter()
        .flat_map(|(m, k)| for_moment(*m).iter().take(*k).map(move |p| build(*p, n)));
    l.sequences = (1..).zip(seqs).collect();
    l
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
        let mut v = serde_json::to_value(seeded_builtin(10)).unwrap();
        edit(&mut v);
        v.to_string()
    }

    const V1: &str = include_str!("../../crates/light-core/tests/fixtures/library-v1.json");

    /// The editor pairs sent and saved entries by index: renumbering never reorders, adds or drops.
    #[test]
    fn assign_ids_keeps_order_and_count_and_changes_only_new_ids() {
        let prev = seeded_builtin(10);
        let mut new = prev.clone();
        let (pal, seq, set) = (
            new.palettes[0].clone(),
            new.sequences[0].clone(),
            new.sets[0].clone(),
        );
        new.palettes.insert(1, (7_000_001, pal.1));
        new.sequences.push((7_000_002, seq.1));
        let mut s = set;
        s.id = 7_000_003;
        s.palettes = vec![7_000_001];
        new.sets.insert(0, s);
        let before = new.clone();
        assign_ids(&prev, &mut new).unwrap();
        let ids = |l: &Library| {
            (
                l.palettes.iter().map(|p| p.0).collect::<Vec<_>>(),
                l.sequences.iter().map(|p| p.0).collect::<Vec<_>>(),
                l.sets.iter().map(|p| p.id).collect::<Vec<_>>(),
            )
        };
        let (sent_p, sent_q, sent_s) = ids(&before);
        let (new_p, new_q, new_s) = ids(&new);
        assert_eq!(
            (sent_p.len(), sent_q.len(), sent_s.len()),
            (new_p.len(), new_q.len(), new_s.len())
        );
        let known = ids(&prev);
        for (a, b) in sent_p
            .iter()
            .zip(&new_p)
            .chain(sent_q.iter().zip(&new_q))
            .chain(sent_s.iter().zip(&new_s))
        {
            let temp = *a >= 7_000_000;
            assert_eq!(a == b, !temp, "only temporary ids change: {a} -> {b}");
        }
        assert!(!known.0.contains(&new_p[1]) && !known.2.contains(&new_s[0]));
        assert_eq!(new.sets[0].palettes, vec![new_p[1]], "references follow");
    }

    #[test]
    fn load_v1_writes_backup_once_and_saves_v3() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, p) = tmp();
        std::fs::write(&p, V1).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
        let lib = load(&p, 4, 8);
        assert_eq!(lib.validate(), Ok(()));
        assert_eq!(lib.sets[0].name, "Club night");
        let backup = dir.path().join("library.json.v1");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), V1);
        let mode = |f: &Path| std::fs::metadata(f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&backup), 0o640, "same permissions as the file");
        assert_eq!(load(&p, 4, 8), lib, "v3 was saved over the original");
        assert!(!dir.path().join("library.json.bad").exists());
        // A second v1 file (e.g. restored by hand) must not replace the first backup.
        std::fs::write(&p, V1.replace("Club night", "Other")).unwrap();
        assert_eq!(load(&p, 4, 8).sets[0].name, "Other");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), V1);
    }

    #[test]
    fn load_v2_writes_backup_once_and_saves_v3() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, p) = tmp();
        let mut v = serde_json::to_value(seeded_builtin(10)).unwrap();
        v["schemaVersion"] = json!(2);
        v["sets"][0]["rhythm"]["sequences"] = json!([{"pattern": "snake"}]);
        let text = v.to_string();
        std::fs::write(&p, &text).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
        let lib = load(&p, 10, 8);
        assert_eq!(lib.validate(), Ok(()));
        assert_eq!(lib.sets[0].rhythm.sequences, [SeqRef::Generated]);
        let backup = dir.path().join("library.json.v2");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), text);
        let mode = |f: &Path| std::fs::metadata(f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&backup), 0o640, "same permissions as the file");
        assert!(
            std::fs::read_to_string(&p)
                .unwrap()
                .contains("\"schemaVersion\":3")
        );
        assert_eq!(load(&p, 10, 8), lib);
        // A second v2 file must not replace the first backup.
        std::fs::write(&p, text.replace("Auto", "Other")).unwrap();
        assert_eq!(load(&p, 10, 8).sets[0].name, "Other");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), text);
        assert!(!dir.path().join("library.json.bad").exists());
    }

    #[test]
    fn failed_migration_leaves_file_untouched() {
        let (dir, p) = tmp();
        let mut v: Value = serde_json::from_str(V1).unwrap();
        v["sets"][0]["rhythm"]["interval"] = json!({"weeks": 1});
        let text = v.to_string();
        std::fs::write(&p, &text).unwrap();
        assert_eq!(load(&p, 4, 8), Library::builtin(4));
        // Kept aside byte for byte like any invalid file; no backup, no half-migrated v2.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("library.json.bad")).unwrap(),
            text
        );
        assert!(!dir.path().join("library.json.v1").exists());
        assert!(!p.exists());
    }

    #[test]
    fn a_second_refs_cleanup_keeps_its_own_copy() {
        let (dir, p) = tmp();
        let text = |n: u32| {
            with(|v| {
                let calm = v["sequences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s[1]["moment"] == "calm")
                    .unwrap()[0]
                    .clone();
                v["sets"][0]["rhythm"]["sequences"] = json!([{ "id": calm }]);
                v["sets"][0]["name"] = json!(format!("set {n}"));
            })
        };
        std::fs::write(&p, text(1)).unwrap();
        let _ = load(&p, 10, 8);
        std::fs::write(&p, text(2)).unwrap();
        let _ = load(&p, 10, 8);
        let read = |n: &str| std::fs::read_to_string(dir.path().join(n)).unwrap();
        assert_eq!(read("library.json.refs"), text(1));
        assert_eq!(read("library.json.refs.1"), text(2));
    }

    #[test]
    fn a_reference_to_another_moments_sequence_is_dropped_on_load() {
        let (dir, p) = tmp();
        let text = with(|v| {
            let calm = v["sequences"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s[1]["moment"] == "calm")
                .unwrap()[0]
                .clone();
            v["sets"][0]["rhythm"]["sequences"] = json!([{ "id": calm }]);
        });
        std::fs::write(&p, &text).unwrap();
        let lib = load(&p, 10, 8);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("library.json.refs")).unwrap(),
            text,
            "the original is kept byte for byte"
        );
        assert_eq!(
            lib.sets[0].rhythm.sequences,
            [light_core::library::SeqRef::Generated]
        );
        assert_eq!(
            lib.sequences,
            seeded_builtin(10).sequences,
            "the rest is kept"
        );
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
            " ".repeat(9 << 20),
        ];
        for text in tampered {
            let (dir, p) = tmp();
            std::fs::write(&p, &text).unwrap();
            assert_eq!(load(&p, 10, 8), Library::builtin(10));
            assert_eq!(
                std::fs::read_to_string(dir.path().join("library.json.bad")).unwrap(),
                text
            );
        }
    }

    #[test]
    fn largest_valid_library_survives_a_save_and_reload() {
        let mut lib = seeded_builtin(60);
        let seq = lib.sequences[0].1.clone();
        let mut big = seq.clone();
        let mut half = seq.steps[0].clone();
        half.len = light_core::model::StepLen::Half; // 128 half beats: 16 bars
        big.steps = vec![half; light_core::library::MAX_STEPS];
        lib.sequences = (1..=64).map(|i| (i, big.clone())).collect();
        lib.sets[0].rhythm.sequences = vec![SeqRef::Id(1)];
        assert_eq!(lib.validate(), Ok(()));
        let (_d, p) = tmp();
        save(&p, &lib).unwrap();
        assert!(
            std::fs::metadata(&p).unwrap().len() > 1 << 20,
            "past the old cap"
        );
        assert_eq!(load(&p, 10, 8), lib);
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
            let _ = load(&p, 10, 8);
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
        assert_eq!(load(&p, 4, 8), Library::builtin(4));
        save(&p, &Library::builtin(6)).unwrap();
        assert_eq!(load(&p, 10, 8), Library::builtin(6));
    }

    #[test]
    fn existing_user_library_is_not_overwritten() {
        let (_d, p) = tmp();
        let mut lib = Library::builtin(6);
        lib.palettes.truncate(1);
        lib.active_palette = Some(lib.palettes[0].0);
        for s in &mut lib.sets {
            s.palettes = vec![lib.palettes[0].0];
        }
        lib.sets[0].name = "Mine".into();
        save(&p, &lib).unwrap();
        assert_eq!(load(&p, 6, 8), lib);
    }

    #[test]
    fn save_rotates_five_backups() {
        let (dir, p) = tmp();
        for n in 1..=8u8 {
            save(&p, &Library::builtin(n)).unwrap();
        }
        assert_eq!(load(&p, 1, 8), Library::builtin(8));
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
        assert_eq!(load(&p, 3, 8), Library::builtin(3));
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            #[test]
            fn arbitrary_bytes_never_panic_and_load_a_valid_library(
                raw in proptest::collection::vec(any::<u8>(), 0..2048),
                segments in 1u8..=60,
            ) {
                let d = tempfile::tempdir().unwrap();
                let p = d.path().join("library.json");
                std::fs::write(&p, &raw).unwrap();
                let lib = load(&p, segments, 8);
                prop_assert!(lib.validate().is_ok());
                // Not a valid library: the built-ins come back and the original is kept aside.
                let valid = serde_json::from_slice::<Library>(&raw).is_ok_and(|l| l.validate().is_ok());
                if !valid {
                    prop_assert_eq!(lib, Library::builtin(segments));
                    prop_assert_eq!(std::fs::read(d.path().join("library.json.bad")).unwrap(), raw);
                }
            }

            #[test]
            fn builtin_libraries_round_trip(segments in 1u8..=60) {
                let d = tempfile::tempdir().unwrap();
                let p = d.path().join("library.json");
                let lib = Library::builtin(segments);
                save(&p, &lib).unwrap();
                prop_assert_eq!(load(&p, segments, 8), lib);
            }
        }
    }
}
