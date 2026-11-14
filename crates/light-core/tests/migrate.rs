#![cfg(feature = "serde")]
#![allow(clippy::expect_used)]
use light_core::library::{Interval, Library, Order, PaletteChange, SeqRef};
use light_core::migrate::{IntervalV1, LibraryV1, LibraryV2, OLD_BUILTIN, migrate_v1, migrate_v2};
use light_core::model::StepLen;

const FIXTURE: &str = include_str!("fixtures/library-v1.json");

fn v1() -> LibraryV1 {
    serde_json::from_str(FIXTURE).expect("fixture parses as v1")
}

#[test]
fn migrates_a_real_v1_library() {
    let lib = migrate_v1(v1(), 8).expect("migrates");
    assert_eq!(lib.validate(), Ok(()));
    assert_eq!(lib.schema_version, 3);
    assert_eq!(lib.active_palette, None);
    assert_eq!(lib.active_set, 1);
    let club = &lib.sets[0];
    assert_eq!(club.name, "Club night");
    assert_eq!(club.palettes, vec![1, 2, 3, 4]);
    assert_eq!(club.palette_order, Order::Random);
    assert_eq!(club.palette_change, PaletteChange::AtDrop);
    assert_eq!(club.rhythm.interval, Interval::Bars(8));
    assert_eq!(club.rhythm.order, Order::Random);
    assert_eq!(
        club.rhythm.sequences,
        vec![SeqRef::Id(1), SeqRef::Id(4), SeqRef::Generated]
    );
    assert_eq!(club.calm.interval, Interval::OncePerBreak);
    assert_eq!(club.buildup.interval, Interval::Bars(2));
    let warm = &lib.sets[1];
    assert_eq!(warm.palettes, vec![3, 1]);
    assert_eq!(warm.palette_order, Order::InOrder);
    assert_eq!(warm.rhythm.interval, Interval::Bars(4));
    assert_eq!(warm.calm.interval, Interval::OncePerBreak);
    // The 3.5-beat "Skip" gained a half-beat step to fill its bar.
    let skip = &lib.sequences[3].1;
    assert_eq!(skip.steps.len(), 5);
    assert_eq!(skip.steps[4].len, StepLen::Half);
    assert_eq!(lib.sequences[0].1.steps.len(), 4, "whole bars untouched");
}

#[test]
fn phrase_interval_uses_phrase_bars() {
    for (pb, want) in [(4u8, 4u16), (8, 8), (16, 16)] {
        let lib = migrate_v1(v1(), pb).expect("migrates");
        assert_eq!(lib.sets[0].rhythm.interval, Interval::Bars(want));
    }
    let mut x = v1();
    x.sets[0].rhythm.interval = IntervalV1::Phrases(2);
    let lib = migrate_v1(x, 16).expect("migrates");
    assert_eq!(lib.sets[0].rhythm.interval, Interval::Bars(16), "capped");
}

#[test]
fn odd_beats_round_up_to_allowed_bars() {
    for (beats, want) in [
        (1u16, 1u16),
        (4, 1),
        (5, 2),
        (9, 4),
        (13, 4),
        (17, 8),
        (20, 8),
        (100, 16),
    ] {
        let mut x = v1();
        x.sets[0].buildup.interval = IntervalV1::Beats(beats);
        let lib = migrate_v1(x, 8).expect("migrates");
        assert_eq!(
            lib.sets[0].buildup.interval,
            Interval::Bars(want),
            "{beats} beats"
        );
    }
    let mut x = v1();
    x.sets[0].rhythm.interval = IntervalV1::Bars(3);
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.sets[0].rhythm.interval, Interval::Bars(4));
}

#[test]
fn pads_three_and_a_half_beats_to_one_bar_looping() {
    let lib = migrate_v1(v1(), 8).expect("migrates");
    let lens: Vec<_> = lib.sequences[3].1.steps.iter().map(|s| s.len).collect();
    assert_eq!(
        lens,
        [
            StepLen::Half,
            StepLen::One,
            StepLen::One,
            StepLen::One,
            StepLen::Half
        ]
    );
}

#[test]
fn fixed_with_many_palettes_becomes_in_order() {
    let mut x = v1();
    x.sets[0].rhythm.order = Order::Fixed;
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.sets[0].palette_order, Order::InOrder);
    let mut x = v1();
    x.sets[0].rhythm.order = Order::Fixed;
    let set = &mut x.sets[0];
    for m in [&mut set.rhythm, &mut set.calm, &mut set.buildup] {
        m.palettes = vec![2];
    }
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(
        lib.sets[0].palette_order,
        Order::Fixed,
        "one palette keeps Fixed"
    );
}

#[test]
fn a_library_that_cannot_validate_is_an_error() {
    let mut x = v1();
    x.active_set = 999;
    assert!(migrate_v1(x, 8).is_err());
}

fn half_beats(l: &Library, i: usize) -> u32 {
    l.sequences[i]
        .1
        .steps
        .iter()
        .map(|s| u32::from(s.len.half_beats()))
        .sum()
}

#[test]
fn truncates_sequences_over_sixteen_bars() {
    let mut x = v1();
    let step = x.sequences[0].1.steps[0].clone();
    x.sequences[0].1.steps = vec![
        light_core::model::Step {
            len: StepLen::Four,
            ..step
        };
        64
    ];
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.validate(), Ok(()));
    assert_eq!(half_beats(&lib, 0), 128);
    assert_eq!(lib.sequences[0].1.steps.len(), 16);
    assert!(
        lib.sequences[0]
            .1
            .steps
            .iter()
            .all(|s| s.len == StepLen::Four)
    );
}

#[test]
fn odd_long_sequence_pads_or_truncates_to_valid() {
    let mut x = v1();
    let step = x.sequences[0].1.steps[0].clone();
    x.sequences[0].1.steps = vec![
        light_core::model::Step {
            len: StepLen::Half,
            ..step.clone()
        };
        37
    ];
    // 127 half beats of Four then a Two: the crossing step is shortened to fit.
    let mut long = vec![
        light_core::model::Step {
            len: StepLen::Four,
            ..step.clone()
        };
        15
    ];
    long.push(light_core::model::Step {
        len: StepLen::Four,
        ..step.clone()
    });
    long.push(light_core::model::Step {
        len: StepLen::Two,
        ..step
    });
    x.sequences[1].1.steps = long;
    let lib = migrate_v1(x, 8).expect("migrates");
    assert_eq!(lib.validate(), Ok(()));
    assert_eq!(half_beats(&lib, 0), 40, "37 padded to 5 bars");
    assert_eq!(half_beats(&lib, 1), 128, "132 cut to 16 bars");
    assert_eq!(lib.sequences[1].1.steps.len(), 16);
}

// ---- schema 2 -> 3 ----

use serde_json::{Value, json};

fn old_palettes() -> Vec<Value> {
    let hsv = |(h, s, v): (u16, u16, u16)| json!({"h": h, "s": s, "v": v});
    OLD_BUILTIN
        .iter()
        .enumerate()
        .map(|(i, (n, a, b, c))| {
            json!([i + 1, {"name": n, "principal": hsv(*a), "contraste": hsv(*b), "suave": hsv(*c)}])
        })
        .collect()
}

fn track(seqs: &Value, interval: &Value) -> Value {
    json!({"sequences": seqs, "order": "inOrder", "interval": interval})
}

/// What an existing user file holds: the old `Library::builtin(10)` (palettes 1-8, set "Auto").
fn v2_json() -> Value {
    let auto = json!({
        "id": 1, "name": "Auto",
        "rhythm": track(&json!(["generated"]), &json!({"bars": 8})),
        "calm": track(&json!(["generated"]), &json!("oncePerBreak")),
        "buildup": track(&json!(["generated"]), &json!({"bars": 2})),
        "palettes": [1, 2, 3, 4, 5, 6, 7, 8], "paletteOrder": "inOrder",
        "paletteChange": "atDrop",
    });
    json!({
        "schemaVersion": 2,
        "shape": serde_json::to_value(Library::builtin(10).shape).expect("shape"),
        "palettes": old_palettes(), "sequences": [],
        "sets": [auto], "activeSet": 1, "activePalette": null,
    })
}

fn migrate(v: &Value) -> Library {
    let v2: LibraryV2 = serde_json::from_value(v.clone()).expect("parses as v2");
    migrate_v2(v2).expect("migrates")
}

fn names(l: &Library) -> Vec<&str> {
    l.palettes.iter().map(|(_, p)| p.name.as_str()).collect()
}

const NEW: [&str; 9] = [
    "Ocean",
    "Laser",
    "UV",
    "Deep sea",
    "Ember",
    "Green",
    "Lavender",
    "Sorbet",
    "Peach and mint",
];

#[test]
fn builtin_palettes_are_the_nine() {
    let ps = light_core::builtin::palettes();
    assert_eq!(ps.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), NEW);
    assert_eq!(
        ps[1].suave,
        light_core::paint::Hsv {
            h: 160,
            s: 600,
            v: 1000
        }
    );
    let l = Library::builtin(10);
    assert_eq!(l.validate(), Ok(()));
    assert_eq!(l.schema_version, 3);
    assert_eq!(l.sets[0].name, "Auto");
    assert_eq!(l.sets[0].palettes, (1..=9).collect::<Vec<u32>>());
}

#[test]
fn v2_pattern_refs_become_generated_even_unknown() {
    let mut v = v2_json();
    v["sets"][0]["rhythm"]["sequences"] =
        json!([{"pattern": "snake"}, {"pattern": "strobe"}, {"id": 1}]);
    v["sets"][0]["calm"]["sequences"] = json!([{"pattern": "nope"}]);
    v["sets"][0]["buildup"]["sequences"] = json!(["generated", {"pattern": "strobe"}]);
    v["sequences"] = json!([[1, {"name": "S", "moment": "rhythm",
        "steps": [{"len": "four", "roles": vec!["principal"; 10]}]}]]);
    let l = migrate(&v);
    let s = &l.sets[0];
    assert_eq!(s.rhythm.sequences, [SeqRef::Generated, SeqRef::Id(1)]);
    assert_eq!(s.calm.sequences, [SeqRef::Generated]);
    assert_eq!(s.buildup.sequences, [SeqRef::Generated]);
    assert!(l.sequences.iter().all(|(_, q)| q.fade), "fade reads on");
}

#[test]
fn untouched_auto_set_gets_the_new_palettes() {
    let l = migrate(&v2_json());
    assert_eq!(names(&l), NEW);
    assert_eq!(l.palettes[0].0, 2, "Ocean keeps its old id");
    let ids: Vec<u32> = l.palettes.iter().map(|p| p.0).collect();
    assert_eq!(l.sets[0].palettes, ids);
}

#[test]
fn used_and_user_palettes_stay() {
    let mut v = v2_json();
    let mut other = v["sets"][0].clone();
    other["id"] = json!(2);
    other["name"] = json!("Mine set");
    other["palettes"] = json!([1]); // Sunset
    v["sets"].as_array_mut().expect("arr").push(other);
    v["activePalette"] = json!(3); // Club
    v["palettes"].as_array_mut().expect("arr").push(json!(
        [20, {"name": "Mine", "principal": {"h": 1, "s": 2, "v": 3},
              "contraste": {"h": 100, "s": 2, "v": 3}, "suave": {"h": 1, "s": 2, "v": 3}}]
    ));
    let l = migrate(&v);
    let mut got = names(&l);
    got.sort_unstable();
    let mut want: Vec<&str> = NEW.to_vec();
    want.extend(["Sunset", "Club", "Mine"]);
    want.sort_unstable();
    assert_eq!(got, want);
    assert_eq!(l.sets[1].palettes, vec![1]);
    assert_eq!(l.active_palette, Some(3));
}

#[test]
fn edited_old_builtin_is_a_user_palette() {
    let mut v = v2_json();
    v["palettes"][4][1]["principal"]["h"] = json!(121);
    let l = migrate(&v);
    assert!(names(&l).contains(&"Forest"));
    assert_eq!(names(&l).len(), 10);
}

#[test]
fn migration_respects_max_palettes() {
    let mut v = v2_json();
    let pal = v["palettes"].as_array_mut().expect("arr");
    pal.clear();
    for i in 1..=30u32 {
        pal.push(
            json!([i, {"name": format!("U{i}"), "principal": {"h": 1, "s": 2, "v": 3},
            "contraste": {"h": 100, "s": 2, "v": 3}, "suave": {"h": 1, "s": 2, "v": 3}}]),
        );
    }
    let hsv = |(h, s, v): (u16, u16, u16)| json!({"h": h, "s": s, "v": v});
    for (k, (n, a, b, c)) in OLD_BUILTIN.iter().take(2).enumerate() {
        pal.push(json!([31 + k, {"name": n, "principal": hsv(*a), "contraste": hsv(*b), "suave": hsv(*c)}]));
    }
    v["sets"][0]["palettes"] = json!([31, 32]);
    let l = migrate(&v);
    assert_eq!(l.palettes.len(), 32);
}

#[test]
fn migrating_twice_does_not_duplicate_palettes() {
    let l = migrate(&v2_json());
    let mut back = serde_json::to_value(&l).expect("ser");
    back["schemaVersion"] = json!(2);
    assert_eq!(migrate(&back).palettes, l.palettes);
}

#[test]
fn v1_chains_to_v3() {
    let lib = migrate_v1(v1(), 8).expect("migrates");
    assert_eq!(lib.schema_version, 3);
    assert_eq!(lib.validate(), Ok(()));
    assert!(names(&lib).contains(&"Laser"), "new palettes added");
}
