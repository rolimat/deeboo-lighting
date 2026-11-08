//! Codec for the strip's segment-paint data point (DP 61).

use std::fmt;

pub const PAINT_DP: u32 = 61;

const MAX_SEGMENTS: usize = 127;

/// Hue 0..=360, saturation and value 0..=1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Hsv {
    pub h: u16,
    pub s: u16,
    pub v: u16,
}

/// Segment numbers are 1-based.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaintCommand {
    Bucket(Hsv),
    Pencil { color: Hsv, segments: Vec<u8> },
    Eraser { segments: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaintError {
    TooShort,
    BadHeader,
    EmptySelection,
    TooManySegments,
    SegmentOutOfRange { segment: u8, count: u8 },
    ColourOutOfRange,
}

impl fmt::Display for PaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "paint payload too short"),
            Self::BadHeader => write!(f, "invalid paint header"),
            Self::EmptySelection => write!(f, "no segments selected"),
            Self::TooManySegments => write!(f, "more than {MAX_SEGMENTS} segments selected"),
            Self::SegmentOutOfRange { segment, count } => {
                write!(f, "segment {segment} outside 1..={count}")
            }
            Self::ColourOutOfRange => write!(f, "colour component out of range"),
        }
    }
}

impl std::error::Error for PaintError {}

/// Encodes a paint command for a strip with `segment_count` segments.
///
/// # Errors
/// Returns a [`PaintError`] for an invalid colour, segment count or selection.
pub fn encode(segment_count: u8, cmd: &PaintCommand) -> Result<Vec<u8>, PaintError> {
    let (action, color, segments) = match cmd {
        PaintCommand::Bucket(c) => (0x00, *c, None),
        PaintCommand::Pencil { color, segments } => (0x01, *color, Some(segments)),
        PaintCommand::Eraser { segments } => (0x02, Hsv { h: 0, s: 0, v: 0 }, Some(segments)),
    };
    if color.h > 360 || color.s > 1000 || color.v > 1000 {
        return Err(PaintError::ColourOutOfRange);
    }
    if segment_count == 0 {
        return Err(PaintError::BadHeader);
    }
    if let Some(segs) = segments {
        if segs.is_empty() {
            return Err(PaintError::EmptySelection);
        }
        if segs.len() > MAX_SEGMENTS {
            return Err(PaintError::TooManySegments);
        }
        if let Some(&segment) = segs.iter().find(|&&s| s == 0 || s > segment_count) {
            return Err(PaintError::SegmentOutOfRange {
                segment,
                count: segment_count,
            });
        }
    }
    let mut out = vec![0x00, 0x01, 0x00, segment_count, action];
    for part in [color.h, color.s, color.v] {
        out.extend_from_slice(&part.to_be_bytes());
    }
    if let Some(segs) = segments {
        #[allow(clippy::cast_possible_truncation)] // len <= 127 checked above
        out.push(0x80 | segs.len() as u8);
        out.extend_from_slice(segs);
    }
    Ok(out)
}

/// Reads the strip's segment count from a DP 61 payload.
///
/// # Errors
/// Returns [`PaintError::TooShort`] or [`PaintError::BadHeader`] for malformed payloads.
pub fn decode_segment_count(payload: &[u8]) -> Result<u8, PaintError> {
    match payload {
        [0x00, 0x01 | 0x02, _, count @ 1..=255, ..] => Ok(*count),
        [_, _, _, _, ..] => Err(PaintError::BadHeader),
        _ => Err(PaintError::TooShort),
    }
}
