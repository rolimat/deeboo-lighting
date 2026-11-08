use light_core::model::Colour;
use light_core::paint::{Hsv, PaintCommand};
use light_core::scene::{apply, diff};
use proptest::prelude::*;

const A: Hsv = Hsv {
    h: 0,
    s: 1000,
    v: 500,
};
const B: Hsv = Hsv {
    h: 120,
    s: 1000,
    v: 500,
};
const C: Hsv = Hsv {
    h: 240,
    s: 1000,
    v: 500,
};

#[test]
fn identical_is_empty() {
    let s = [Some(A), None, Some(B)];
    assert_eq!(diff(&s, &s), Vec::<PaintCommand>::new());
}

#[test]
fn all_same_colour_is_one_bucket() {
    assert_eq!(
        diff(&[Some(A), Some(B), None], &[Some(A); 3]),
        vec![PaintCommand::Bucket(A)]
    );
    assert_eq!(
        diff(&[Some(A); 3], &[Some(B); 3]),
        vec![PaintCommand::Bucket(B)]
    );
}

#[test]
fn all_off_is_one_eraser() {
    assert_eq!(
        diff(&[Some(A), None, Some(B)], &[None; 3]),
        vec![PaintCommand::Eraser {
            segments: vec![1, 2, 3]
        }]
    );
}

#[test]
fn off_first_then_one_pencil_per_colour() {
    let prev = [Some(A), Some(A), None, Some(B), Some(C)];
    let next = [Some(B), None, Some(A), Some(B), Some(A)];
    assert_eq!(
        diff(&prev, &next),
        vec![
            PaintCommand::Eraser { segments: vec![2] },
            PaintCommand::Pencil {
                color: B,
                segments: vec![1]
            },
            PaintCommand::Pencil {
                color: A,
                segments: vec![3, 5]
            },
        ]
    );
}

fn colour() -> impl Strategy<Value = Colour> {
    prop_oneof![
        Just(None),
        Just(Some(A)),
        Just(Some(B)),
        Just(Some(C)),
        Just(Some(Hsv {
            h: 60,
            s: 0,
            v: 1000
        }))
    ]
}

proptest! {
    #[test]
    fn apply_of_diff_reaches_next(
        (prev, next) in (1usize..=32).prop_flat_map(|n| (
            proptest::collection::vec(colour(), n),
            proptest::collection::vec(colour(), n),
        ))
    ) {
        let cmds = diff(&prev, &next);
        let mut state = prev.clone();
        for c in &cmds { apply(&mut state, c); }
        prop_assert_eq!(&state, &next);
        let mut seen = Vec::new();
        for (i, c) in cmds.iter().enumerate() {
            match c {
                PaintCommand::Eraser { .. } => prop_assert_eq!(i, 0),
                PaintCommand::Pencil { color, .. } | PaintCommand::Bucket(color) => {
                    prop_assert!(!seen.contains(color));
                    seen.push(*color);
                }
            }
        }
    }
}
