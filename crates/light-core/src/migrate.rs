//! Library schema 1 → 2 → 3 (redesign spec §6.5). The old shapes live here only to be read once.
use crate::builtin;
use crate::library::{
    INTERVAL_BARS, Interval, Library, LibraryError, MAX_PALETTES, MomentSet, Order, PaletteChange,
    SCHEMA_VERSION, SeqRef, Set, Shape, pad_to_bars,
};
use crate::model::{Palette, Sequence};
use crate::paint::Hsv;

/// Schema 2: sets referenced patterns by name; any name is accepted, it all becomes `Generated`.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SeqRefV2 {
    Id(u32),
    Generated,
    Pattern(String),
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MomentSetV2 {
    pub sequences: Vec<SeqRefV2>,
    pub order: Order,
    pub interval: Interval,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetV2 {
    pub id: u32,
    pub name: String,
    pub rhythm: MomentSetV2,
    pub calm: MomentSetV2,
    pub buildup: MomentSetV2,
    pub palettes: Vec<u32>,
    pub palette_order: Order,
    pub palette_change: PaletteChange,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryV2 {
    pub schema_version: u16,
    pub shape: Shape,
    pub palettes: Vec<(u32, Palette)>,
    pub sequences: Vec<(u32, Sequence)>,
    pub sets: Vec<SetV2>,
    pub active_set: u32,
    pub active_palette: Option<u32>,
}

type Hsv3 = (u16, u16, u16);

/// The 8 v1/v2 built-in palettes (name, principal, contraste, suave), in the order of their ids 1-8.
pub const OLD_BUILTIN: [(&str, Hsv3, Hsv3, Hsv3); 8] = [
    ("Sunset", (20, 950, 1000), (280, 900, 950), (30, 300, 1000)),
    (
        "Ocean",
        (190, 1000, 1000),
        (320, 900, 1000),
        (190, 300, 1000),
    ),
    (
        "Club",
        (300, 1000, 1000),
        (180, 1000, 1000),
        (300, 250, 1000),
    ),
    (
        "Fire and ice",
        (10, 1000, 1000),
        (210, 1000, 1000),
        (20, 300, 1000),
    ),
    ("Forest", (120, 900, 900), (300, 800, 1000), (120, 300, 950)),
    (
        "Aurora",
        (160, 900, 1000),
        (270, 900, 1000),
        (200, 250, 1000),
    ),
    (
        "Candy",
        (330, 900, 1000),
        (180, 800, 1000),
        (330, 250, 1000),
    ),
    ("Gold", (45, 1000, 1000), (240, 900, 1000), (45, 300, 1000)),
];

fn is_old_builtin(p: &Palette) -> bool {
    let t = |h: Hsv| (h.h, h.s, h.v);
    OLD_BUILTIN.iter().any(|(n, a, b, c)| {
        p.name == *n && t(p.principal) == *a && t(p.contraste) == *b && t(p.suave) == *c
    })
}

fn track(m: MomentSetV2) -> MomentSet {
    let mut sequences: Vec<SeqRef> = Vec::new();
    for r in m.sequences {
        let r = match r {
            SeqRefV2::Id(i) => SeqRef::Id(i),
            SeqRefV2::Generated | SeqRefV2::Pattern(_) => SeqRef::Generated,
        };
        if !(r == SeqRef::Generated && sequences.last() == Some(&r)) {
            sequences.push(r);
        }
    }
    MomentSet {
        sequences,
        order: m.order,
        interval: m.interval,
    }
}

/// Old patterns become "Any pattern"; unused old built-in palettes give way to the 9 new ones.
/// An untouched "Auto" set (palettes = ids 1-8) is rebuilt on the new palettes.
///
/// # Errors
/// When the migrated library does not pass `validate` (the caller keeps the original file).
pub fn migrate_v2(v2: LibraryV2) -> Result<Library, LibraryError> {
    let old_ids: Vec<u32> = (1..=8).collect();
    let untouched = |s: &SetV2| s.name == "Auto" && s.palettes == old_ids;
    let mut used: Vec<u32> = v2.active_palette.into_iter().collect();
    for s in v2.sets.iter().filter(|s| !untouched(s)) {
        used.extend(&s.palettes);
    }
    let new = builtin::palettes();
    let mut next = v2.palettes.iter().map(|p| p.0).max().unwrap_or(0);
    let mut palettes: Vec<(u32, Palette)> = v2
        .palettes
        .into_iter()
        .filter(|(id, p)| used.contains(id) || !is_old_builtin(p) || new.iter().any(|n| n == p))
        .collect();
    for n in new {
        if palettes.len() >= MAX_PALETTES {
            break;
        }
        if !palettes.iter().any(|(_, p)| p.name == n.name) {
            next = next.saturating_add(1);
            palettes.push((next, n));
        }
    }
    let new_ids: Vec<u32> = builtin::palettes()
        .iter()
        .filter_map(|n| {
            palettes
                .iter()
                .find(|(_, p)| p.name == n.name)
                .map(|(i, _)| *i)
        })
        .collect();
    let sets = v2
        .sets
        .into_iter()
        .map(|s| {
            let auto = untouched(&s);
            let mut kept: Vec<u32> = s
                .palettes
                .iter()
                .copied()
                .filter(|id| palettes.iter().any(|(i, _)| i == id))
                .collect();
            if auto {
                let rest = kept.into_iter().filter(|id| !new_ids.contains(id));
                kept = new_ids.iter().copied().chain(rest).collect();
            } else if kept.is_empty() {
                kept = new_ids
                    .first()
                    .or(palettes.first().map(|p| &p.0))
                    .copied()
                    .into_iter()
                    .collect();
            }
            Set {
                id: s.id,
                name: s.name,
                rhythm: track(s.rhythm),
                calm: track(s.calm),
                buildup: track(s.buildup),
                palettes: kept,
                palette_order: s.palette_order,
                palette_change: s.palette_change,
            }
        })
        .collect();
    let mut lib = Library {
        schema_version: SCHEMA_VERSION,
        shape: v2.shape,
        palettes,
        sequences: v2.sequences,
        sets,
        active_set: v2.active_set,
        active_palette: v2.active_palette,
    };
    let _ = lib.drop_foreign_refs(); // a v2 file is backed up whole by the caller
    lib.validate()?;
    Ok(lib)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntervalV1 {
    Beats(u16),
    Bars(u16),
    Phrases(u16),
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MomentSetV1 {
    pub sequences: Vec<SeqRefV1>,
    pub palettes: Vec<u32>,
    pub order: Order,
    pub interval: IntervalV1,
}

/// v1 had no pattern references.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SeqRefV1 {
    Id(u32),
    Generated,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetV1 {
    pub id: u32,
    pub name: String,
    pub rhythm: MomentSetV1,
    pub calm: MomentSetV1,
    pub buildup: MomentSetV1,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryV1 {
    pub schema_version: u16,
    pub shape: Shape,
    pub palettes: Vec<(u32, Palette)>,
    pub sequences: Vec<(u32, Sequence)>,
    pub sets: Vec<SetV1>,
    pub active_set: u32,
    pub active_palette: Option<u32>,
}

/// Rounds up to the next allowed bar count, capped at 16.
fn to_bars(bars: u32) -> u16 {
    INTERVAL_BARS
        .into_iter()
        .find(|b| u32::from(*b) >= bars)
        .unwrap_or(16)
}

fn interval(i: IntervalV1, phrase_bars: u8) -> Interval {
    Interval::Bars(to_bars(match i {
        IntervalV1::Phrases(n) => u32::from(n) * u32::from(phrase_bars),
        IntervalV1::Beats(n) => u32::from(n).div_ceil(4),
        IntervalV1::Bars(n) => u32::from(n),
    }))
}

fn moment(m: &MomentSetV1, phrase_bars: u8) -> MomentSetV2 {
    MomentSetV2 {
        sequences: m
            .sequences
            .iter()
            .map(|r| match *r {
                SeqRefV1::Id(i) => SeqRefV2::Id(i),
                SeqRefV1::Generated => SeqRefV2::Generated,
            })
            .collect(),
        order: m.order,
        interval: interval(m.interval, phrase_bars),
    }
}

/// v1 → v2 shapes → `migrate_v2`.
///
/// # Errors
/// When the migrated library does not pass `validate` (the caller keeps the original file).
pub fn migrate_v1(v1: LibraryV1, phrase_bars: u8) -> Result<Library, LibraryError> {
    let sets = v1
        .sets
        .iter()
        .map(|s| {
            let mut palettes: Vec<u32> = Vec::new();
            for p in s
                .rhythm
                .palettes
                .iter()
                .chain(&s.calm.palettes)
                .chain(&s.buildup.palettes)
            {
                if !palettes.contains(p) {
                    palettes.push(*p);
                }
            }
            let palette_order = match s.rhythm.order {
                Order::Fixed if palettes.len() > 1 => Order::InOrder,
                o => o,
            };
            SetV2 {
                id: s.id,
                name: s.name.clone(),
                rhythm: moment(&s.rhythm, phrase_bars),
                calm: MomentSetV2 {
                    interval: Interval::OncePerBreak,
                    ..moment(&s.calm, phrase_bars)
                },
                buildup: moment(&s.buildup, phrase_bars),
                palettes,
                palette_order,
                palette_change: PaletteChange::AtDrop,
            }
        })
        .collect();
    let mut sequences = v1.sequences;
    for (_, s) in &mut sequences {
        pad_to_bars(s);
    }
    migrate_v2(LibraryV2 {
        schema_version: 2,
        shape: v1.shape,
        palettes: v1.palettes,
        sequences,
        sets,
        active_set: v1.active_set,
        active_palette: None,
    })
}
