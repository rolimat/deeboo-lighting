//! Library schema 1 → 2 (redesign spec §6.5). The v1 shapes live here only to be read once.
use crate::library::{
    BAR_HALF_BEATS, INTERVAL_BARS, Interval, Library, LibraryError, MAX_STEPS, MomentSet, Order,
    PaletteChange, SCHEMA_VERSION, SeqRef, Set, Shape,
};
use crate::model::{Palette, Sequence};

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

fn moment(m: &MomentSetV1, phrase_bars: u8) -> MomentSet {
    MomentSet {
        sequences: m
            .sequences
            .iter()
            .map(|r| match *r {
                SeqRefV1::Id(i) => SeqRef::Id(i),
                SeqRefV1::Generated => SeqRef::Generated,
            })
            .collect(),
        order: m.order,
        interval: interval(m.interval, phrase_bars),
    }
}

/// Appends steps repeating from the first until the sequence fills whole bars.
fn pad_to_bars(seq: &mut Sequence) {
    let total = |s: &Sequence| -> u32 {
        s.steps
            .iter()
            .map(|st| u32::from(st.len.half_beats()))
            .sum()
    };
    let n = seq.steps.len();
    let mut i = 0;
    // The length cap stops a runaway; `validate` then rejects the sequence.
    while n > 0 && total(seq) % BAR_HALF_BEATS != 0 && seq.steps.len() <= MAX_STEPS {
        seq.steps.push(seq.steps[i % n].clone());
        i += 1;
    }
}

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
            Set {
                id: s.id,
                name: s.name.clone(),
                rhythm: moment(&s.rhythm, phrase_bars),
                calm: MomentSet {
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
    let lib = Library {
        schema_version: SCHEMA_VERSION,
        shape: v1.shape,
        palettes: v1.palettes,
        sequences,
        sets,
        active_set: v1.active_set,
        active_palette: None,
    };
    lib.validate()?;
    Ok(lib)
}
