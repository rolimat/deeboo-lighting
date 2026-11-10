#![allow(clippy::expect_used)]
use light_core::groups::{Groups, groups};
use light_core::library::{Piece, Shape};
use proptest::prelude::*;

fn at(x: f32, y: f32) -> Piece {
    Piece {
        x,
        y,
        angle: 0,
        len: 1,
    }
}

fn partitions<const K: usize>(g: &[Vec<u8>; K], n: u8) -> bool {
    let mut all: Vec<u8> = g.iter().flatten().copied().collect();
    all.sort_unstable();
    all == (1..=n).collect::<Vec<_>>()
}

fn check(g: &Groups, n: u8) -> bool {
    partitions(&g.left_right, n)
        && partitions(&g.top_bottom, n)
        && partitions(&g.centre_ends, n)
        && partitions(&g.odd_even, n)
        && partitions(&g.quarters, n)
        && g.along == (1..=n).collect::<Vec<_>>()
}

#[test]
fn row_shape_halves_and_quarters() {
    let g = groups(&Shape::row(8));
    assert_eq!(g.left_right, [vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
    assert_eq!(g.odd_even, [vec![1, 3, 5, 7], vec![2, 4, 6, 8]]);
    assert_eq!(g.quarters, [vec![1, 2], vec![3, 4], vec![5, 6], vec![7, 8]]);
    assert_eq!(g.centre_ends[0], vec![3, 4, 5, 6]);
    assert_eq!(g.centre_ends[1], vec![1, 2, 7, 8]);
    // odd N: the extra segment goes to the first half
    assert_eq!(
        groups(&Shape::row(5)).left_right,
        [vec![1, 2, 3], vec![4, 5]]
    );
}

#[test]
fn square_shape_left_right_and_top_bottom() {
    let s = Shape {
        pieces: vec![at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0), at(1.0, 1.0)],
    };
    let g = groups(&s);
    assert_eq!(g.left_right, [vec![1, 3], vec![2, 4]]);
    assert_eq!(g.top_bottom, [vec![1, 2], vec![3, 4]]);
}

#[test]
fn single_segment_is_valid() {
    let g = groups(&Shape::row(1));
    assert!(check(&g, 1));
    assert_eq!(g.along, vec![1]);
}

proptest! {
    #[test]
    fn groups_partition_segments(
        ps in prop::collection::vec((0u8..=64, 0u8..=64, 0u8..4, 1u8..=4), 1..=60)
    ) {
        let pieces: Vec<Piece> = ps.iter()
            .map(|&(x, y, a, len)| Piece { x: f32::from(x), y: f32::from(y), angle: u16::from(a) * 90, len })
            .collect();
        let n = u8::try_from(pieces.len()).expect("<= 60");
        let g = groups(&Shape { pieces });
        prop_assert!(check(&g, n));
    }
}

#[test]
fn off_axis_pieces_use_their_true_centre() {
    // The 315-degree piece rises to the right: its centre is above the flat one's.
    let flat = Piece {
        x: 0.0,
        y: 9.7,
        angle: 0,
        len: 1,
    };
    let up = Piece {
        x: 10.0,
        y: 10.0,
        angle: 315,
        len: 4,
    };
    let g = groups(&Shape {
        pieces: vec![up, flat],
    });
    assert_eq!(g.top_bottom[0], vec![1]);
}
