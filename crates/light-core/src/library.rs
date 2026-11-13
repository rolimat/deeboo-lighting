//! The user library: shape, palettes, sequences and sets, with strict validation (plan 5).
use crate::builtin::{self, test_palette};
use crate::detect::Moment;
use crate::model::{Palette, Sequence};
use crate::paint::Hsv;
use crate::patterns::Pattern;
use std::fmt;

pub const SCHEMA_VERSION: u16 = 2;
pub const MAX_PALETTES: usize = 32;
pub const MAX_SEQUENCES: usize = 64;
pub const MAX_SETS: usize = 16;
pub const MAX_STEPS: usize = 128;
pub const MAX_BARS: u16 = 16;
/// Half beats in a bar.
pub const BAR_HALF_BEATS: u32 = 8;
/// Bar counts allowed for intervals and palette changes.
pub const INTERVAL_BARS: [u16; 5] = [1, 2, 4, 8, 16];
pub const MAX_NAME_CHARS: usize = 63;
pub const GRID: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Piece {
    pub x: f32,
    pub y: f32,
    pub angle: u16,
    pub len: u8,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Shape {
    pub pieces: Vec<Piece>,
}

impl Shape {
    /// `n` pieces of one cell, side by side on the grid.
    #[must_use]
    pub fn row(n: u8) -> Self {
        Shape {
            pieces: (0..n)
                .map(|i| Piece {
                    x: f32::from(i),
                    y: 0.0,
                    angle: 0,
                    len: 1,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Order {
    InOrder,
    Random,
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Interval {
    Bars(u16),
    /// Calm only: advances once each time the moment is entered.
    OncePerBreak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum SeqRef {
    Id(u32),
    /// Any pattern of the moment, freshly generated.
    Generated,
    /// This pattern, with new parameters each time.
    Pattern(Pattern),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum PaletteChange {
    AtDrop,
    Bars(u16),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct MomentSet {
    pub sequences: Vec<SeqRef>,
    pub order: Order,
    pub interval: Interval,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Set {
    pub id: u32,
    pub name: String,
    pub rhythm: MomentSet,
    pub calm: MomentSet,
    pub buildup: MomentSet,
    pub palettes: Vec<u32>,
    pub palette_order: Order,
    pub palette_change: PaletteChange,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Library {
    pub schema_version: u16,
    pub shape: Shape,
    pub palettes: Vec<(u32, Palette)>,
    pub sequences: Vec<(u32, Sequence)>,
    pub sets: Vec<Set>,
    pub active_set: u32,
    /// `None` = Auto: follow the active set's palette rotation.
    pub active_palette: Option<u32>,
}

/// Names the offending field and a fixed reason; never carries user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryError {
    pub field: String,
    pub reason: &'static str,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid library: {} {}", self.field, self.reason)
    }
}

impl std::error::Error for LibraryError {}

fn err<T>(field: impl Into<String>, reason: &'static str) -> Result<T, LibraryError> {
    Err(LibraryError {
        field: field.into(),
        reason,
    })
}

fn check_name(field: String, name: &str) -> Result<(), LibraryError> {
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME_CHARS {
        return err(field, "must be 1 to 63 characters");
    }
    if name.chars().any(char::is_control) {
        return err(field, "must not contain control characters");
    }
    Ok(())
}

fn check_hsv(field: String, c: Hsv) -> Result<(), LibraryError> {
    if c.h > 360 || c.s > 1000 || c.v > 1000 {
        return err(field, "is outside H 0-360, S/V 0-1000");
    }
    Ok(())
}

/// A palette that arrives outside a library (the editor's "Show on strip" preview).
///
/// # Errors
/// The first field that breaks the same limits a saved palette has.
pub fn validate_palette(p: &Palette) -> Result<(), LibraryError> {
    check_name("palette.name".into(), &p.name)?;
    check_hsv("palette.principal".into(), p.principal)?;
    check_hsv("palette.contraste".into(), p.contraste)?;
    check_hsv("palette.suave".into(), p.suave)
}

fn unique_ids<'a>(field: &str, ids: impl Iterator<Item = &'a u32>) -> Result<(), LibraryError> {
    let mut seen = std::collections::HashSet::new();
    if ids.into_iter().all(|id| seen.insert(*id)) {
        Ok(())
    } else {
        err(field, "has a duplicate id")
    }
}

impl Library {
    /// Removes references to another moment's sequence (a track left empty falls back to
    /// `Generated`): old files may hold them and `validate` rejects them. Returns whether
    /// anything changed.
    pub fn drop_foreign_refs(&mut self) -> bool {
        let mut changed = false;
        let seqs = &self.sequences;
        for set in &mut self.sets {
            for (m, moment) in [
                (&mut set.rhythm, Moment::Rhythm),
                (&mut set.calm, Moment::Calm),
                (&mut set.buildup, Moment::BuildUp),
            ] {
                let before = m.sequences.len();
                m.sequences.retain(|r| match r {
                    SeqRef::Id(id) => !seqs.iter().any(|(s, q)| s == id && q.moment != moment),
                    _ => true,
                });
                changed |= m.sequences.len() != before;
                if m.sequences.is_empty() && before > 0 {
                    m.sequences.push(SeqRef::Generated);
                }
            }
        }
        changed
    }

    /// First-run library (spec §6.3): the 8 built-in palettes, the 10 built-in sequences for a row of
    /// `segments`, and the active set "Auto". A stored library always has palettes and sequences, so
    /// built-ins are never merged into one; `library_store::load` only uses this when none is usable.
    #[must_use]
    pub fn builtin(segments: u8) -> Self {
        let shape = Shape::row(segments);
        let seqs = builtin::sequences(segments, &crate::groups::groups(&shape));
        let palettes: Vec<(u32, Palette)> = (1..).zip(builtin::palettes()).collect();
        let ids: Vec<u32> = palettes.iter().map(|(i, _)| *i).collect();
        Library {
            schema_version: SCHEMA_VERSION,
            shape,
            palettes,
            sequences: (1..).zip(seqs).collect(),
            sets: vec![builtin::auto_set(1, &ids)],
            active_set: 1,
            active_palette: None,
        }
    }

    /// The active set, if it exists.
    #[must_use]
    pub fn current_set(&self) -> Option<&Set> {
        self.sets.iter().find(|s| s.id == self.active_set)
    }

    /// The palette in force: the chosen one, else the active set's first.
    #[must_use]
    pub fn current_palette_id(&self) -> Option<u32> {
        self.active_palette
            .or_else(|| self.current_set()?.palettes.first().copied())
    }

    /// The palette to show; the test palette when the library has none.
    #[must_use]
    pub fn current_palette(&self) -> Palette {
        self.current_palette_id()
            .and_then(|id| self.palettes.iter().find(|(i, _)| *i == id))
            .map_or_else(test_palette, |(_, p)| p.clone())
    }

    /// Segment count the library was built for (the shape's piece count).
    #[must_use]
    pub fn segments(&self) -> u8 {
        u8::try_from(self.shape.pieces.len()).unwrap_or(u8::MAX)
    }

    /// Gives every step exactly `m` roles: missing ones are `Off`, extra ones are dropped. Used when
    /// the shape's piece count changes (a re-paired strip); the shape itself is not touched.
    pub fn resize_segments(&mut self, m: u8) {
        for (_, s) in &mut self.sequences {
            for st in &mut s.steps {
                st.roles.resize(usize::from(m), crate::model::Role::Off);
            }
        }
    }

    /// Independent of the strip's current segment count: a re-paired strip keeps the library.
    ///
    /// # Errors
    /// The first violated limit, naming the field.
    #[allow(clippy::too_many_lines)] // one flat list of limits
    pub fn validate(&self) -> Result<(), LibraryError> {
        if self.schema_version != SCHEMA_VERSION {
            return err("schema_version", "is not supported");
        }
        if self.palettes.len() > MAX_PALETTES {
            return err("palettes", "has too many entries");
        }
        if self.sequences.len() > MAX_SEQUENCES {
            return err("sequences", "has too many entries");
        }
        if self.sets.len() > MAX_SETS {
            return err("sets", "has too many entries");
        }
        unique_ids("palettes", self.palettes.iter().map(|(i, _)| i))?;
        unique_ids("sequences", self.sequences.iter().map(|(i, _)| i))?;
        unique_ids("sets", self.sets.iter().map(|s| &s.id))?;

        let segments = usize::from(self.segments());
        if !(1..=60).contains(&self.shape.pieces.len()) {
            return err("shape", "must have 1 to 60 pieces");
        }
        for (i, p) in self.shape.pieces.iter().enumerate() {
            let at = |f: &str| format!("shape.pieces[{i}].{f}");
            for (name, v) in [("x", p.x), ("y", p.y)] {
                if !v.is_finite() || !(0.0..=GRID).contains(&v) {
                    return err(at(name), "must be finite and within the 0-64 grid");
                }
            }
            if p.angle >= 360 || p.angle % 15 != 0 {
                return err(at("angle"), "must be a multiple of 15 below 360");
            }
            if !(1..=4).contains(&p.len) {
                return err(at("len"), "must be 1 to 4 cells");
            }
        }
        for (i, (_, p)) in self.palettes.iter().enumerate() {
            check_name(format!("palettes[{i}].name"), &p.name)?;
            check_hsv(format!("palettes[{i}].principal"), p.principal)?;
            check_hsv(format!("palettes[{i}].contraste"), p.contraste)?;
            check_hsv(format!("palettes[{i}].suave"), p.suave)?;
        }
        for (i, (_, s)) in self.sequences.iter().enumerate() {
            check_name(format!("sequences[{i}].name"), &s.name)?;
            if s.steps.is_empty() || s.steps.len() > MAX_STEPS {
                return err(format!("sequences[{i}].steps"), "must have 1 to 128 steps");
            }
            let half_beats: u32 = s
                .steps
                .iter()
                .map(|st| u32::from(st.len.half_beats()))
                .sum();
            if !half_beats.is_multiple_of(BAR_HALF_BEATS)
                || half_beats > u32::from(MAX_BARS) * BAR_HALF_BEATS
            {
                return err(
                    format!("sequences[{i}].steps"),
                    "must last a whole number of bars, 16 at most",
                );
            }
            if let Some(j) = s.steps.iter().position(|st| st.roles.len() != segments) {
                return err(
                    format!("sequences[{i}].steps[{j}].roles"),
                    "must have one role per segment",
                );
            }
        }
        let has_pal = |id: &u32| self.palettes.iter().any(|(p, _)| p == id);
        let has_seq = |r: &SeqRef, m: Moment| match r {
            SeqRef::Generated => true,
            SeqRef::Pattern(p) => p.moment() == m,
            SeqRef::Id(id) => self.sequences.iter().any(|(s, q)| s == id && q.moment == m),
        };
        for (i, set) in self.sets.iter().enumerate() {
            check_name(format!("sets[{i}].name"), &set.name)?;
            for (n, m, moment) in [
                ("rhythm", &set.rhythm, Moment::Rhythm),
                ("calm", &set.calm, Moment::Calm),
                ("buildup", &set.buildup, Moment::BuildUp),
            ] {
                match m.interval {
                    Interval::Bars(v) if INTERVAL_BARS.contains(&v) => {}
                    Interval::OncePerBreak if moment == Moment::Calm => {}
                    _ => return err(format!("sets[{i}].{n}.interval"), "is not allowed here"),
                }
                if m.sequences.len() > MAX_SEQUENCES {
                    return err(format!("sets[{i}].{n}.sequences"), "has too many entries");
                }
                if m.sequences.is_empty() {
                    return err(
                        format!("sets[{i}].{n}.sequences"),
                        "needs at least one sequence",
                    );
                }
                if !m.sequences.iter().all(|r| has_seq(r, moment)) {
                    return err(
                        format!("sets[{i}].{n}.sequences"),
                        "references an unknown sequence",
                    );
                }
            }
            if set.palettes.len() > MAX_PALETTES {
                return err(format!("sets[{i}].palettes"), "has too many entries");
            }
            if set.palettes.is_empty() {
                return err(format!("sets[{i}].palettes"), "needs at least one palette");
            }
            if !set.palettes.iter().all(has_pal) {
                return err(
                    format!("sets[{i}].palettes"),
                    "references an unknown palette",
                );
            }
            if matches!(set.palette_change, PaletteChange::Bars(v) if !INTERVAL_BARS.contains(&v)) {
                return err(format!("sets[{i}].palette_change"), "is not allowed");
            }
        }
        if !self.sets.iter().any(|s| s.id == self.active_set) {
            return err("active_set", "references an unknown set");
        }
        if self.active_palette.is_some_and(|p| !has_pal(&p)) {
            return err("active_palette", "references an unknown palette");
        }
        Ok(())
    }
}
