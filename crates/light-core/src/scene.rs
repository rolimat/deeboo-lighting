//! Scene diff: previous sent state to the minimal DP 61 commands (spec §6.5).

use crate::model::Colour;
use crate::paint::{Hsv, PaintCommand};

fn seg(i: usize) -> u8 {
    // Segment counts are bounded by the codec (<= 127), so this cannot truncate.
    u8::try_from(i + 1).unwrap_or(u8::MAX)
}

/// Commands that turn `prev` into `next`: erase first, then one pencil per
/// target colour (ordered by lowest segment). Unchanged segments are skipped.
pub fn diff(prev: &[Colour], next: &[Colour]) -> Vec<PaintCommand> {
    if prev == next {
        return Vec::new();
    }
    if let Some(Some(c)) = next.first()
        && next.iter().all(|n| *n == Some(*c))
    {
        return vec![PaintCommand::Bucket(*c)];
    }
    if next.iter().all(Option::is_none) {
        return vec![PaintCommand::Eraser {
            segments: (0..next.len()).map(seg).collect(),
        }];
    }
    let mut out = Vec::new();
    let off: Vec<u8> = (0..next.len())
        .filter(|&i| next[i].is_none() && prev[i].is_some())
        .map(seg)
        .collect();
    if !off.is_empty() {
        out.push(PaintCommand::Eraser { segments: off });
    }
    let mut groups: Vec<(Hsv, Vec<u8>)> = Vec::new();
    for (i, n) in next.iter().enumerate() {
        let Some(c) = *n else { continue };
        if prev[i] == Some(c) {
            continue;
        }
        match groups.iter_mut().find(|(g, _)| *g == c) {
            Some((_, s)) => s.push(seg(i)),
            None => groups.push((c, vec![seg(i)])),
        }
    }
    out.extend(
        groups
            .into_iter()
            .map(|(color, segments)| PaintCommand::Pencil { color, segments }),
    );
    out
}

/// Applies a command to a mirror of the strip (index 0 = segment 1).
pub fn apply(state: &mut [Colour], cmd: &PaintCommand) {
    match cmd {
        PaintCommand::Bucket(c) => state.fill(Some(*c)),
        PaintCommand::Pencil { color, segments } => set(state, segments, Some(*color)),
        PaintCommand::Eraser { segments } => set(state, segments, None),
    }
}

fn set(state: &mut [Colour], segments: &[u8], c: Colour) {
    for &s in segments {
        if let Some(slot) = usize::from(s).checked_sub(1).and_then(|i| state.get_mut(i)) {
            *slot = c;
        }
    }
}
