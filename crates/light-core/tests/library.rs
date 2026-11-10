#![allow(clippy::expect_used)]
use light_core::builtin::test_palette;
use light_core::detect::Moment;
use light_core::library::{Interval, Library, PaletteChange, Piece, SeqRef, Shape};
use light_core::model::{Role, Step, StepLen};
use light_core::patterns::Pattern;

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
fn builtin_auto_set_uses_spec_intervals() {
    let l = Library::builtin(8);
    let s = &l.sets[0];
    assert_eq!(s.name, "Auto");
    assert_eq!(s.rhythm.interval, Interval::Bars(8));
    assert_eq!(s.calm.interval, Interval::OncePerBreak);
    assert_eq!(s.buildup.interval, Interval::Bars(2));
    assert_eq!(s.palette_change, PaletteChange::AtDrop);
    assert_eq!(l.active_palette, None);
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
                l.sequences[0].1.steps = vec![st; 129];
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
        ("sets[0].palettes", Box::new(|l| l.sets[0].palettes.clear())),
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
            Box::new(|l| l.shape.pieces[0].angle = 50),
        ),
        (
            "shape.pieces[0].angle",
            Box::new(|l| l.shape.pieces[0].angle = 360),
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
            "sets[0].rhythm.sequences",
            Box::new(|l| l.sets[0].rhythm.sequences = vec![SeqRef::Generated; 65]),
        ),
        (
            "sets[0].palettes",
            Box::new(|l| l.sets[0].palettes = vec![l.palettes[0].0; 33]),
        ),
        (
            "sets[0].rhythm.interval",
            Box::new(|l| l.sets[0].rhythm.interval = Interval::Bars(0)),
        ),
        ("active_set", Box::new(|l| l.active_set = 999)),
        ("active_palette", Box::new(|l| l.active_palette = Some(999))),
        ("schema_version", Box::new(|l| l.schema_version = 1)),
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
    l.sets[0].palettes.push(999);
    assert_eq!(fields(&l), "sets[0].palettes");
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
    assert!(json.contains("\"schemaVersion\":2"), "{json}");
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

#[test]
fn resize_segments_pads_with_off_and_truncates() {
    use light_core::model::Role;
    for m in [4u8, 12, 60] {
        let mut l = Library::builtin(10);
        l.shape = Shape::row(m);
        assert!(l.validate().is_err(), "roles still have 10 entries (m={m})");
        let before = l.clone();
        l.resize_segments(m);
        assert_eq!(l.validate(), Ok(()), "m={m}");
        for ((_, a), (_, b)) in before.sequences.iter().zip(&l.sequences) {
            for (x, y) in a.steps.iter().zip(&b.steps) {
                let keep = usize::from(m).min(10);
                assert_eq!(x.roles[..keep], y.roles[..keep]);
                assert!(y.roles[keep..].iter().all(|r| *r == Role::Off));
            }
        }
    }
}

#[test]
fn palettes_combine() {
    let ps = light_core::builtin::palettes();
    assert_eq!(ps.len(), 8);
    let names: std::collections::HashSet<_> = ps.iter().map(|p| p.name.clone()).collect();
    assert_eq!(names.len(), 8);
    for p in &ps {
        let d = p.principal.h.abs_diff(p.contraste.h);
        assert!(d.min(360 - d) >= 60, "{}", p.name);
        assert!(p.suave.s <= 450 && p.suave.v >= 800, "{}", p.name);
    }
}

#[test]
fn ten_sequences_validate_for_common_n() {
    use light_core::detect::Moment::{BuildUp, Calm, Rhythm};
    for n in [1u8, 4, 6, 9, 10, 12, 16, 60] {
        let g = light_core::groups::groups(&Shape::row(n));
        let seqs = light_core::builtin::sequences(n, &g);
        assert_eq!(seqs.len(), 10);
        let count = |m| seqs.iter().filter(|s| s.moment == m).count();
        assert_eq!((count(Rhythm), count(Calm), count(BuildUp)), (4, 3, 3));
        let names: std::collections::HashSet<_> = seqs.iter().map(|s| &s.name).collect();
        assert_eq!(names.len(), 10, "n={n}");
        for s in &seqs {
            assert!(s.validate(n).is_ok(), "{} n={n}", s.name);
        }
    }
}

#[test]
fn first_run_seeds_auto_set() {
    let l = Library::builtin(10);
    assert_eq!((l.palettes.len(), l.sequences.len()), (8, 10));
    let s = l.current_set().expect("active");
    assert_eq!(s.name, "Auto");
    assert_eq!(s.palettes.len(), 8);
    for m in [&s.rhythm, &s.calm, &s.buildup] {
        assert_eq!(m.sequences, vec![SeqRef::Generated]);
    }
}

fn steps(n: usize, len: StepLen, segments: usize) -> Vec<Step> {
    vec![
        Step {
            len,
            roles: vec![Role::Off; segments]
        };
        n
    ]
}

#[test]
fn rejects_sequence_not_whole_bars() {
    let mut l = Library::builtin(4);
    let mut st = steps(3, StepLen::One, 4);
    st.insert(
        0,
        Step {
            len: StepLen::Half,
            roles: vec![Role::Off; 4],
        },
    );
    l.sequences[0].1.steps = st; // 1 + 2 + 2 + 2 = 7 half beats
    assert_eq!(fields(&l), "sequences[0].steps");
}

#[test]
fn accepts_sixteen_bars_rejects_seventeen() {
    let mut l = Library::builtin(4);
    l.sequences[0].1.steps = steps(16, StepLen::Four, 4);
    assert_eq!(l.validate(), Ok(()));
    l.sequences[0].1.steps = steps(17, StepLen::Four, 4);
    assert_eq!(fields(&l), "sequences[0].steps");
}

#[test]
fn once_per_break_only_on_calm() {
    let mut l = Library::builtin(4);
    l.sets[0].calm.interval = Interval::OncePerBreak;
    assert_eq!(l.validate(), Ok(()));
    l.sets[0].rhythm.interval = Interval::OncePerBreak;
    assert_eq!(fields(&l), "sets[0].rhythm.interval");
    l.sets[0].rhythm.interval = Interval::Bars(3);
    assert_eq!(fields(&l), "sets[0].rhythm.interval");
    l.sets[0].rhythm.interval = Interval::Bars(16);
    l.sets[0].buildup.interval = Interval::OncePerBreak;
    assert_eq!(fields(&l), "sets[0].buildup.interval");
}

#[test]
fn pattern_ref_must_match_track_moment() {
    let mut l = Library::builtin(4);
    l.sets[0].rhythm.sequences = vec![SeqRef::Pattern(Pattern::Snake)];
    l.sets[0].calm.sequences = vec![SeqRef::Pattern(Pattern::Drift)];
    l.sets[0].buildup.sequences = vec![SeqRef::Pattern(Pattern::Strobe)];
    assert_eq!(l.validate(), Ok(()));
    l.sets[0].calm.sequences = vec![SeqRef::Pattern(Pattern::Snake)];
    assert_eq!(fields(&l), "sets[0].calm.sequences");
}

#[test]
fn set_palettes_required_and_known() {
    let mut l = Library::builtin(4);
    l.sets[0].palettes.clear();
    assert_eq!(fields(&l), "sets[0].palettes");
    let mut l = Library::builtin(4);
    l.sets[0].palettes = vec![999];
    assert_eq!(fields(&l), "sets[0].palettes");
    let mut l = Library::builtin(4);
    l.sets[0].palette_change = PaletteChange::Bars(3);
    assert_eq!(fields(&l), "sets[0].palette_change");
    l.sets[0].palette_change = PaletteChange::Bars(4);
    assert_eq!(l.validate(), Ok(()));
}

#[test]
fn pattern_labels_are_english_and_distinct() {
    let all: Vec<_> = [Moment::Rhythm, Moment::Calm, Moment::BuildUp]
        .iter()
        .flat_map(|m| light_core::patterns::for_moment(*m))
        .collect();
    assert_eq!(all.len(), 11);
    let names: std::collections::HashSet<_> = all.iter().map(|p| p.label()).collect();
    assert_eq!(names.len(), 11);
    assert_eq!(Pattern::EndsCentre.label(), "Ends \u{2194} centre");
}

#[test]
fn builtin_library_validates_for_n() {
    for n in [1u8, 8, 10, 60] {
        assert_eq!(Library::builtin(n).validate(), Ok(()), "n={n}");
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trip_v2() {
    use serde_json::json;
    fn j<T: serde::Serialize>(v: T) -> serde_json::Value {
        serde_json::to_value(v).expect("ser")
    }
    assert_eq!(j(Interval::Bars(4)), json!({"bars": 4}));
    assert_eq!(j(Interval::OncePerBreak), json!("oncePerBreak"));
    assert_eq!(j(SeqRef::Id(3)), json!({"id": 3}));
    assert_eq!(j(SeqRef::Generated), json!("generated"));
    assert_eq!(
        j(SeqRef::Pattern(Pattern::Halves)),
        json!({"pattern": "halves"})
    );
    assert_eq!(j(PaletteChange::AtDrop), json!("atDrop"));
    assert_eq!(j(PaletteChange::Bars(2)), json!({"bars": 2}));
    let mut l = Library::builtin(6);
    l.sets[0].rhythm.sequences = vec![SeqRef::Pattern(Pattern::OddEven), SeqRef::Id(1)];
    l.sets[0].palette_change = PaletteChange::Bars(8);
    let back: Library = serde_json::from_value(j(&l)).expect("de");
    assert_eq!(back, l);
    let set = j(&l.sets[0]);
    assert!(set.get("palettes").is_some() && set["rhythm"].get("palettes").is_none());
}

#[test]
fn validate_palette_checks_name_and_ranges() {
    use light_core::library::validate_palette;
    use light_core::model::Palette;
    use light_core::paint::Hsv;
    let ok = Palette {
        name: "Neon".into(),
        principal: Hsv {
            h: 360,
            s: 1000,
            v: 1000,
        },
        contraste: Hsv { h: 0, s: 0, v: 0 },
        suave: Hsv {
            h: 10,
            s: 10,
            v: 10,
        },
    };
    assert!(validate_palette(&ok).is_ok());
    let mut bad = ok.clone();
    bad.suave.v = 1001;
    assert!(validate_palette(&bad).is_err());
    bad = ok.clone();
    bad.principal.h = 361;
    assert!(validate_palette(&bad).is_err());
    bad = ok;
    bad.name = String::new();
    assert!(validate_palette(&bad).is_err());
}

#[test]
fn angles_in_15_degree_steps_validate() {
    let mut l = Library::builtin(3);
    for a in (0..360).step_by(15) {
        l.shape.pieces[0].angle = a;
        assert_eq!(l.validate(), Ok(()), "angle {a}");
    }
}
