#![allow(clippy::unwrap_used)] // helper fn is outside allow-unwrap-in-tests
use light_core::paint::{Hsv, PaintCommand, PaintError, decode_segment_count, encode};
use proptest::prelude::*;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
const RED: Hsv = Hsv {
    h: 0,
    s: 1000,
    v: 1000,
};
const WHITE: Hsv = Hsv {
    h: 0,
    s: 0,
    v: 1000,
};

#[test]
fn bucket_red() {
    assert_eq!(
        encode(10, &PaintCommand::Bucket(RED)).unwrap(),
        hex("0001000a00000003e803e8")
    );
}
#[test]
fn bucket_blue() {
    let blue = Hsv {
        h: 240,
        s: 1000,
        v: 1000,
    };
    assert_eq!(
        encode(10, &PaintCommand::Bucket(blue)).unwrap(),
        hex("0001000a0000f003e803e8")
    );
}
#[test]
fn pencil_red_odd() {
    let cmd = PaintCommand::Pencil {
        color: RED,
        segments: vec![1, 3, 5, 7, 9],
    };
    assert_eq!(
        encode(10, &cmd).unwrap(),
        hex("0001000a01000003e803e8850103050709")
    );
}
#[test]
fn pencil_white_even() {
    let cmd = PaintCommand::Pencil {
        color: WHITE,
        segments: vec![2, 4, 6, 8, 10],
    };
    assert_eq!(
        encode(10, &cmd).unwrap(),
        hex("0001000a010000000003e885020406080a")
    );
}
#[test]
fn eraser_segment_8() {
    let cmd = PaintCommand::Eraser { segments: vec![8] };
    assert_eq!(encode(10, &cmd).unwrap(), hex("0001000a020000000000008108"));
}
#[test]
fn decodes_count_from_smart_life_capture() {
    assert_eq!(
        decode_segment_count(&hex("0002000a01001e032003e88108")).unwrap(),
        10
    );
}
#[test]
fn rejects_segment_zero_and_above_count() {
    let cmd = PaintCommand::Pencil {
        color: RED,
        segments: vec![11],
    };
    assert!(matches!(
        encode(10, &cmd),
        Err(PaintError::SegmentOutOfRange {
            segment: 11,
            count: 10
        })
    ));
    assert!(
        encode(
            10,
            &PaintCommand::Pencil {
                color: RED,
                segments: vec![0]
            }
        )
        .is_err()
    );
}
#[test]
fn rejects_empty_selection_and_bad_colour() {
    assert_eq!(
        encode(10, &PaintCommand::Eraser { segments: vec![] }),
        Err(PaintError::EmptySelection)
    );
    assert_eq!(
        encode(10, &PaintCommand::Bucket(Hsv { h: 361, s: 0, v: 0 })),
        Err(PaintError::ColourOutOfRange)
    );
}
#[test]
fn rejects_more_than_127_segments() {
    let segs: Vec<u8> = (1..=128).collect();
    assert_eq!(
        encode(200, &PaintCommand::Eraser { segments: segs }),
        Err(PaintError::TooManySegments)
    );
}
#[test]
fn rejects_zero_segment_count() {
    assert_eq!(
        encode(0, &PaintCommand::Bucket(RED)),
        Err(PaintError::BadHeader)
    );
}
#[test]
fn rejects_short_and_bad_header() {
    assert_eq!(decode_segment_count(&[0, 1, 0]), Err(PaintError::TooShort));
    assert_eq!(
        decode_segment_count(&[1, 1, 0, 5]),
        Err(PaintError::BadHeader)
    );
    assert_eq!(
        decode_segment_count(&[0, 3, 0, 5]),
        Err(PaintError::BadHeader)
    );
}

proptest! {
    #[test]
    fn decode_never_panics(bytes in any::<Vec<u8>>()) {
        let _ = decode_segment_count(&bytes);
    }
    #[test]
    fn count_round_trips(n in 1u8..=255) {
        prop_assert_eq!(decode_segment_count(&encode(n, &PaintCommand::Bucket(RED)).unwrap()), Ok(n));
    }
}

#[test]
fn decode_rejects_zero_count() {
    assert_eq!(
        decode_segment_count(&[0, 1, 0, 0]),
        Err(PaintError::BadHeader)
    );
}
