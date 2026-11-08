#![allow(clippy::expect_used)]
use light_core::builtin::test_palette;
use light_core::library::{Interval, Library, Piece, SeqRef, Shape};

#[test]
fn builtin_validates_for_1_to_60_segments() {
    for n in 1..=60u8 {
        let l = Library::builtin(n);
        assert_eq!(l.validate(), Ok(()), "n={n}");
        assert_eq!(l.segments(), n);
    }
}

#[test]
fn shape_row_has_n_pieces() {
    let s = Shape::row(7);
    assert_eq!(s.pieces.len(), 7);
    assert!(s.pieces.iter().all(|p| p.len >= 1 && p.angle == 0));
}

#[test]
fn builtin_default_set_uses_spec_intervals() {
    let l = Library::builtin(8);
    let s = &l.sets[0];
    assert_eq!(s.name, "Default");
    assert_eq!(s.rhythm.interval, Interval::Phrases(1));
    assert_eq!(s.buildup.interval, Interval::Bars(2));
    assert_eq!(l.active_set, s.id);
}

fn fields(l: &Library) -> String {
    l.validate().expect_err("must be rejected").field
}

type Mutate = Box<dyn Fn(&mut Library)>;

#[test]
#[allow(clippy::too_many_lines)] // one table
fn validate_rejects_each_limit() {
    let base = Library::builtin(8);
    let long = "x".repeat(64);
    let cases: Vec<(&str, Mutate)> = vec![
        (
            "palettes",
            Box::new(|l| {
                for i in 0..33 {
                    l.palettes.push((100 + i, test_palette()));
                }
            }),
        ),
        (
            "sequences",
            Box::new(|l| {
                for i in 0..65 {
                    let s = l.sequences[0].1.clone();
                    l.sequences.push((100 + i, s));
                }
            }),
        ),
        (
            "sets",
            Box::new(|l| {
                for i in 0..16 {
                    let s = l.sets[0].clone();
                    l.sets.push(light_core::library::Set { id: 100 + i, ..s });
                }
            }),
        ),
        (
            "sequences[0].steps",
            Box::new(|l| {
                let st = l.sequences[0].1.steps[0].clone();
                l.sequences[0].1.steps = vec![st; 65];
            }),
        ),
        (
            "sequences[0].steps",
            Box::new(|l| l.sequences[0].1.steps.clear()),
        ),
        (
            "sequences[0].steps[0].roles",
            Box::new(|l| {
                l.sequences[0].1.steps[0].roles.pop();
            }),
        ),
        (
            "palettes[0].name",
            Box::new(move |l| l.palettes[0].1.name = long.clone()),
        ),
        (
            "palettes[0].name",
            Box::new(|l| l.palettes[0].1.name.clear()),
        ),
        (
            "palettes[0].name",
            Box::new(|l| l.palettes[0].1.name = "a\nb".into()),
        ),
        (
            "sequences[0].name",
            Box::new(|l| l.sequences[0].1.name = "a\u{7}".into()),
        ),
        ("sets[0].name", Box::new(|l| l.sets[0].name.clear())),
        (
            "palettes[0].principal",
            Box::new(|l| l.palettes[0].1.principal.h = 361),
        ),
        (
            "palettes[0].contraste",
            Box::new(|l| l.palettes[0].1.contraste.s = 1001),
        ),
        (
            "palettes[0].suave",
            Box::new(|l| l.palettes[0].1.suave.v = 1001),
        ),
        (
            "sequences[0].steps[0].roles",
            Box::new(|l| {
                l.shape.pieces.pop();
            }),
        ),
        ("shape", Box::new(|l| l.shape.pieces.clear())),
        (
            "shape",
            Box::new(|l| l.shape.pieces = vec![l.shape.pieces[0]; 61]),
        ),
        (
            "sets[0].rhythm.sequences",
            Box::new(|l| l.sets[0].rhythm.sequences.clear()),
        ),
        (
            "sets[0].calm.palettes",
            Box::new(|l| l.sets[0].calm.palettes.clear()),
        ),
        (
            "shape.pieces[0].x",
            Box::new(|l| l.shape.pieces[0].x = f32::NAN),
        ),
        (
            "shape.pieces[0].y",
            Box::new(|l| l.shape.pieces[0].y = f32::INFINITY),
        ),
        (
            "shape.pieces[0].x",
            Box::new(|l| l.shape.pieces[0].x = 64.5),
        ),
        (
            "shape.pieces[0].y",
            Box::new(|l| l.shape.pieces[0].y = -0.5),
        ),
        (
            "shape.pieces[0].angle",
            Box::new(|l| l.shape.pieces[0].angle = 45),
        ),
        (
            "shape.pieces[0].len",
            Box::new(|l| l.shape.pieces[0].len = 0),
        ),
        (
            "shape.pieces[0].len",
            Box::new(|l| l.shape.pieces[0].len = 5),
        ),
        (
            "palettes",
            Box::new(|l| {
                let p = l.palettes[0].clone();
                l.palettes.push(p);
            }),
        ),
        (
            "sets[0].rhythm.interval",
            Box::new(|l| l.sets[0].rhythm.interval = Interval::Bars(0)),
        ),
        ("active_set", Box::new(|l| l.active_set = 999)),
        ("active_palette", Box::new(|l| l.active_palette = Some(999))),
        ("schema_version", Box::new(|l| l.schema_version = 2)),
    ];
    for (i, (field, mutate)) in cases.iter().enumerate() {
        let mut l = base.clone();
        mutate(&mut l);
        assert_eq!(fields(&l), *field, "case {i}");
    }
}

#[test]
fn library_for_10_validates_and_reports_10() {
    let l = Library::builtin(10);
    assert_eq!(l.validate(), Ok(()));
    assert_eq!(l.segments(), 10);
}

#[test]
fn roles_must_match_shape_not_strip() {
    let mut l = Library::builtin(10);
    l.sequences[1].1.steps[0].roles.pop();
    assert_eq!(fields(&l), "sequences[1].steps[0].roles");
}

#[test]
fn set_must_reference_existing_ids() {
    let mut l = Library::builtin(4);
    l.sets[0].calm.sequences.push(SeqRef::Id(999));
    assert_eq!(fields(&l), "sets[0].calm.sequences");
    let mut l = Library::builtin(4);
    l.sets[0].buildup.palettes.push(999);
    assert_eq!(fields(&l), "sets[0].buildup.palettes");
    let mut l = Library::builtin(4);
    l.sets[0].rhythm.sequences.push(SeqRef::Generated);
    assert_eq!(l.validate(), Ok(()));
}

#[test]
fn error_never_echoes_input() {
    let mut l = Library::builtin(4);
    l.palettes[0].1.name = "secret\n".repeat(50);
    let e = l.validate().expect_err("bad name");
    assert!(!format!("{e} {e:?}").contains("secret"));
}

#[test]
fn piece_fields_are_public() {
    let p = Piece {
        x: 1.0,
        y: 2.0,
        angle: 90,
        len: 2,
    };
    assert_eq!(p.len, 2);
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trip() {
    let l = Library::builtin(8);
    let json = serde_json::to_string(&l).expect("ser");
    assert!(json.contains("\"schemaVersion\":1"), "{json}");
    assert!(json.contains("\"activeSet\""), "{json}");
    let back: Library = serde_json::from_str(&json).expect("de");
    assert_eq!(back, l);
}

#[cfg(feature = "serde")]
#[test]
fn unknown_fields_are_refused() {
    let mut v = serde_json::to_value(Library::builtin(4)).expect("ser");
    v["extra"] = 1.into();
    assert!(serde_json::from_value::<Library>(v).is_err());
    let mut v = serde_json::to_value(Library::builtin(4)).expect("ser");
    v["shape"]["pieces"][0]["extra"] = 1.into();
    assert!(serde_json::from_value::<Library>(v).is_err());
}
