//! The user library: shape, palettes, sequences and sets, with strict validation (plan 5).
use crate::builtin::{sequence, test_palette};
use crate::detect::Moment;
use crate::model::{Palette, Sequence};
use crate::paint::Hsv;
use std::fmt;

pub const SCHEMA_VERSION: u16 = 1;
pub const MAX_PALETTES: usize = 32;
pub const MAX_SEQUENCES: usize = 64;
pub const MAX_SETS: usize = 16;
pub const MAX_STEPS: usize = 64;
pub const MAX_NAME_CHARS: usize = 63;
pub const GRID: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Piece {
    pub x: f32,
    pub y: f32,
    pub angle: u16,
    pub len: u8,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
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
    Beats(u16),
    Bars(u16),
    Phrases(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum SeqRef {
    Id(u32),
    Generated,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct MomentSet {
    pub sequences: Vec<SeqRef>,
    pub palettes: Vec<u32>,
    pub order: Order,
    pub interval: Interval,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Set {
    pub id: u32,
    pub name: String,
    pub rhythm: MomentSet,
    pub calm: MomentSet,
    pub buildup: MomentSet,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Library {
    pub schema_version: u16,
    pub shape: Shape,
    pub palettes: Vec<(u32, Palette)>,
    pub sequences: Vec<(u32, Sequence)>,
    pub sets: Vec<Set>,
    pub active_set: u32,
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

fn unique_ids<'a>(field: &str, ids: impl Iterator<Item = &'a u32>) -> Result<(), LibraryError> {
    let mut seen = std::collections::HashSet::new();
    if ids.into_iter().all(|id| seen.insert(*id)) {
        Ok(())
    } else {
        err(field, "has a duplicate id")
    }
}

impl Library {
    /// Test palette, the three built-in sequences and a "Default" set (spec §6.3).
    #[must_use]
    pub fn builtin(segments: u8) -> Self {
        let set_of = |moment, interval| {
            let id = match moment {
                Moment::Rhythm => 1,
                Moment::Calm => 2,
                Moment::BuildUp => 3,
            };
            MomentSet {
                sequences: vec![SeqRef::Id(id)],
                palettes: vec![1],
                order: Order::InOrder,
                interval,
            }
        };
        Library {
            schema_version: SCHEMA_VERSION,
            shape: Shape::row(segments),
            palettes: vec![(1, test_palette())],
            sequences: [(1, Moment::Rhythm), (2, Moment::Calm), (3, Moment::BuildUp)]
                .into_iter()
                .map(|(id, m)| (id, sequence(m, segments)))
                .collect(),
            sets: vec![Set {
                id: 1,
                name: "Default".into(),
                rhythm: set_of(Moment::Rhythm, Interval::Phrases(1)),
                // "Per break": one pass per calm section.
                calm: set_of(Moment::Calm, Interval::Phrases(1)),
                buildup: set_of(Moment::BuildUp, Interval::Bars(2)),
            }],
            active_set: 1,
            active_palette: Some(1),
        }
    }

    /// # Errors
    /// The first violated limit, naming the field.
    pub fn validate(&self, segments: u8) -> Result<(), LibraryError> {
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

        if self.shape.pieces.len() != usize::from(segments) {
            return err("shape", "must have exactly one piece per segment");
        }
        for (i, p) in self.shape.pieces.iter().enumerate() {
            let at = |f: &str| format!("shape.pieces[{i}].{f}");
            for (name, v) in [("x", p.x), ("y", p.y)] {
                if !v.is_finite() || !(0.0..=GRID).contains(&v) {
                    return err(at(name), "must be finite and within the 0-64 grid");
                }
            }
            if !matches!(p.angle, 0 | 90 | 180 | 270) {
                return err(at("angle"), "must be 0, 90, 180 or 270");
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
                return err(format!("sequences[{i}].steps"), "must have 1 to 64 steps");
            }
            if let Some(j) = s
                .steps
                .iter()
                .position(|st| st.roles.len() != usize::from(segments))
            {
                return err(
                    format!("sequences[{i}].steps[{j}].roles"),
                    "must have one role per segment",
                );
            }
        }
        let has_pal = |id: &u32| self.palettes.iter().any(|(p, _)| p == id);
        let has_seq = |r: &SeqRef| match r {
            SeqRef::Generated => true,
            SeqRef::Id(id) => self.sequences.iter().any(|(s, _)| s == id),
        };
        for (i, set) in self.sets.iter().enumerate() {
            check_name(format!("sets[{i}].name"), &set.name)?;
            for (n, m) in [
                ("rhythm", &set.rhythm),
                ("calm", &set.calm),
                ("buildup", &set.buildup),
            ] {
                let (Interval::Beats(v) | Interval::Bars(v) | Interval::Phrases(v)) = m.interval;
                if v == 0 {
                    return err(format!("sets[{i}].{n}.interval"), "must be at least 1");
                }
                if !m.sequences.iter().all(has_seq) {
                    return err(
                        format!("sets[{i}].{n}.sequences"),
                        "references an unknown sequence",
                    );
                }
                if !m.palettes.iter().all(has_pal) {
                    return err(
                        format!("sets[{i}].{n}.palettes"),
                        "references an unknown palette",
                    );
                }
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
