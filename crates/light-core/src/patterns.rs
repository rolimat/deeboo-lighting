//! Pattern library: step sequences per moment (spec §6.4).
//!
//! Patterns are built from chains of states where each transition is a single
//! `scene::diff` message (bucket, one pencil or one eraser): see `wipe` and `duel`.
//! Moment constraints beat `Params`: Calm always uses `Suave`/`Off` and `StepLen::Four`;
//! Build-up steps are at most `StepLen::One`. Out-of-range params are clamped.
#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::enum_glob_use
)]
use crate::detect::Moment;
use crate::groups::Groups;
use crate::model::{Role, Sequence, Step, StepLen};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Pattern {
    // Rhythm
    Halves,
    EndsCentre,
    OddEven,
    Quarters,
    Snake,
    Mirror,
    // Calm
    Drift,
    Sparse,
    // Build-up
    GroupFlash,
    FastSnake,
    Strobe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub step: StepLen,
    pub roles: (Role, Role),
    pub reverse: bool,
    /// 1..=4, clamped. More = finer chunks / wider windows.
    pub density: u8,
}

impl Pattern {
    /// English name shown in the UI.
    #[must_use]
    pub fn label(self) -> &'static str {
        use Pattern::*;
        match self {
            Halves => "Halves",
            EndsCentre => "Ends \u{2194} centre",
            OddEven => "Odd / even",
            Quarters => "Quarters",
            Snake => "Snake",
            Mirror => "Mirror",
            Drift => "Drift",
            Sparse => "Sparse",
            GroupFlash => "Group flash",
            FastSnake => "Fast snake",
            Strobe => "Strobe",
        }
    }

    /// The moment this pattern is made for.
    #[must_use]
    pub fn moment(self) -> Moment {
        use Pattern::*;
        match self {
            Halves | EndsCentre | OddEven | Quarters | Snake | Mirror => Moment::Rhythm,
            Drift | Sparse => Moment::Calm,
            GroupFlash | FastSnake | Strobe => Moment::BuildUp,
        }
    }
}

#[must_use]
pub fn for_moment(m: Moment) -> &'static [Pattern] {
    use Pattern::*;
    match m {
        Moment::Rhythm => &[Halves, EndsCentre, OddEven, Quarters, Snake, Mirror],
        Moment::Calm => &[Drift, Sparse],
        Moment::BuildUp => &[GroupFlash, FastSnake, Strobe],
    }
}

type State = Vec<Role>;

fn paint(n: usize, base: Role, sets: &[(&[u8], Role)]) -> State {
    let mut s = vec![base; n];
    for (segs, r) in sets {
        for &g in *segs {
            if let Some(slot) = usize::from(g).checked_sub(1).and_then(|i| s.get_mut(i)) {
                *slot = *r;
            }
        }
    }
    s
}

/// Splits `items` into at most `c` contiguous merged groups.
fn merge(items: &[Vec<u8>], c: usize) -> Vec<Vec<u8>> {
    let (len, c) = (items.len(), c.min(items.len()));
    (0..c)
        .map(|k| items[k * len / c..(k + 1) * len / c].concat())
        .collect()
}

/// all `a`, groups turn `b` one by one, all `b`, groups turn back to `a` one by one.
/// Every transition is one message.
fn wipe(groups: &[Vec<u8>], a: Role, b: Role, n: usize) -> Vec<State> {
    let cum = |k: usize, on: Role, rest: Role| {
        let lit: Vec<u8> = groups[..k].concat();
        paint(n, rest, &[(&lit, on)])
    };
    let g = groups.len();
    (0..=g)
        .map(|k| cum(k, b, a))
        .chain((1..g).map(|k| cum(k, a, b)))
        .collect()
}

/// all `a`; `y` turns `b`; all `b`; `y` turns back `a`. One message per transition.
/// (`x` is the complement of `y`; it needs no painting.)
fn duel(y: &[u8], a: Role, b: Role, n: usize) -> Vec<State> {
    vec![
        paint(n, a, &[]),
        paint(n, a, &[(y, b)]),
        paint(n, b, &[]),
        paint(n, b, &[(y, a)]),
    ]
}

fn dedupe(mut v: Vec<State>) -> Vec<State> {
    v.dedup();
    if v.len() > 1 && v.first() == v.last() {
        v.pop();
    }
    v
}

fn singles(n: u8) -> Vec<Vec<u8>> {
    (1..=n).map(|s| vec![s]).collect()
}

fn calm_states(p: Pattern, params: Params, n: u8) -> Vec<State> {
    let nn = usize::from(n);
    let d = usize::from(params.density);
    let on = Role::Suave;
    let lit = |set: &[u8]| paint(nn, Role::Off, &[(set, on)]);
    if p == Pattern::Sparse {
        let m = nn.min(2 + d);
        let mut dots: Vec<u8> = (0..m)
            .map(|i| u8::try_from(i * nn / m + 1).unwrap_or(u8::MAX))
            .collect();
        if params.reverse {
            dots.reverse();
        }
        return (1..=m)
            .map(|k| lit(&dots[..k]))
            .chain((1..=m).map(|k| lit(&dots[k..])))
            .collect();
    }
    let mut units = merge(&singles(n), 8);
    if params.reverse {
        units.reverse();
    }
    let c = units.len();
    if c < 2 {
        return vec![lit(&units.concat()), lit(&[])];
    }
    let w = d.min(c / 2).max(1);
    let window = |i: usize, len: usize| -> State {
        let set: Vec<u8> = (i..i + len).flat_map(|u| units[u % c].clone()).collect();
        lit(&set)
    };
    (0..c)
        .flat_map(|i| [window(i, w), window(i, w + 1)])
        .collect()
}

/// # Panics
/// Never: all indexing is bounds-checked or clamped.
#[must_use]
pub fn build(p: Pattern, params: &Params, groups: &Groups, n: u8) -> Sequence {
    use Pattern::*;
    let nn = usize::from(n);
    let params = Params {
        density: params.density.clamp(1, 4),
        ..*params
    };
    let d = usize::from(params.density);
    let (mut a, mut b) = params.roles;
    if params.reverse {
        (a, b) = (b, a);
    }
    let rev = |mut v: Vec<Vec<u8>>| {
        if params.reverse {
            v.reverse();
        }
        v
    };
    let along = singles(n);
    let (moment, name, len, states) = match p {
        Halves => (
            Moment::Rhythm,
            p.label(),
            params.step,
            duel(&groups.left_right[1], a, b, nn),
        ),
        EndsCentre => (
            Moment::Rhythm,
            p.label(),
            params.step,
            duel(&groups.centre_ends[1], a, b, nn),
        ),
        OddEven => (
            Moment::Rhythm,
            p.label(),
            params.step,
            duel(&groups.odd_even[1], a, b, nn),
        ),
        Quarters => (
            Moment::Rhythm,
            p.label(),
            params.step,
            wipe(
                &rev(groups.quarters.to_vec()),
                params.roles.0,
                params.roles.1,
                nn,
            ),
        ),
        Snake => (
            Moment::Rhythm,
            p.label(),
            params.step,
            wipe(
                &rev(merge(&along, 4 * d)),
                params.roles.0,
                params.roles.1,
                nn,
            ),
        ),
        Mirror => {
            let pairs: Vec<Vec<u8>> = (0..along.len().div_ceil(2))
                .map(|k| {
                    let mut v = along[k].clone();
                    let o = &along[along.len() - 1 - k];
                    if o != &v {
                        v.extend(o);
                    }
                    v
                })
                .collect();
            (
                Moment::Rhythm,
                p.label(),
                params.step,
                wipe(
                    &rev(merge(&pairs, 4 * d)),
                    params.roles.0,
                    params.roles.1,
                    nn,
                ),
            )
        }
        Drift | Sparse => (
            Moment::Calm,
            p.label(),
            StepLen::Four,
            calm_states(p, params, n),
        ),
        GroupFlash => (
            Moment::BuildUp,
            p.label(),
            fast(params.step),
            wipe(
                &rev(groups.quarters.to_vec()),
                Role::Off,
                params.roles.0,
                nn,
            ),
        ),
        FastSnake => (
            Moment::BuildUp,
            p.label(),
            fast(params.step),
            wipe(&rev(merge(&along, 4 * d)), Role::Off, params.roles.0, nn),
        ),
        Strobe => (
            Moment::BuildUp,
            p.label(),
            fast(params.step),
            vec![paint(nn, params.roles.0, &[]), paint(nn, Role::Off, &[])],
        ),
    };
    let mut states = dedupe(states);
    if states.is_empty() {
        states.push(vec![Role::Off; nn]);
    }
    // At most 16 bars (128 half beats): longer cycles get shorter steps.
    let mut len = len;
    while states.len() * usize::from(len.half_beats()) > 128 {
        len = match len {
            StepLen::Four => StepLen::Two,
            StepLen::Two => StepLen::One,
            _ => StepLen::Half,
        };
    }
    let mut steps: Vec<Step> = states
        .into_iter()
        .map(|roles| Step { len, roles })
        .collect();
    whole_bars(&mut steps);
    Sequence {
        name: name.into(),
        moment,
        steps,
    }
}

/// Half beats in a bar.
const BAR: u32 = 8;

/// Makes the loop a whole number of bars. Repeating the cycle keeps the loop identical; a cycle
/// too long for that (over 16 half beats) instead holds its last state for the missing beats.
fn whole_bars(steps: &mut Vec<Step>) {
    let total: u32 = steps.iter().map(|s| u32::from(s.len.half_beats())).sum();
    if total.is_multiple_of(BAR) {
        return;
    }
    let reps = BAR / gcd(total, BAR);
    if total * reps <= 16 * BAR {
        let cycle = steps.clone();
        for _ in 1..reps {
            steps.extend(cycle.iter().cloned());
        }
        return;
    }
    let roles = steps.last().map(|s| s.roles.clone()).unwrap_or_default();
    let mut pad = BAR - total % BAR;
    for (len, hb) in [(StepLen::Two, 4), (StepLen::One, 2), (StepLen::Half, 1)] {
        if pad >= hb {
            steps.push(Step {
                len,
                roles: roles.clone(),
            });
            pad -= hb;
        }
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Build-up steps are at most one beat.
fn fast(s: StepLen) -> StepLen {
    if s.half_beats() > 2 { StepLen::One } else { s }
}
