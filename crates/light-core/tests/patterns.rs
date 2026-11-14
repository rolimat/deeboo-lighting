#![allow(clippy::expect_used, clippy::panic, clippy::match_same_arms)]
use light_core::detect::Moment;
use light_core::model::{Role, Sequence, StepLen};
use light_core::patterns::{Pattern, build, for_moment};

const NS: [u8; 3] = [8, 6, 10];

/// Hand-written region masks (X = in the region) of the spec's parts for N = 8, 6 and 10.
fn mask(n: u8, region: &str) -> &'static str {
    match (n, region) {
        (8, "all") => "XXXXXXXX",
        (8, "p1") => "XX------",
        (8, "p2") => "--XX----",
        (8, "p3") => "----XX--",
        (8, "p4") => "------XX",
        (8, "p13") => "XX--XX--",
        (8, "p24") => "--XX--XX",
        (8, "e") => "XX----XX",
        (8, "c") => "--XXXX--",
        (8, "h1") => "XXXX----",
        (8, "h2") => "----XXXX",
        (8, "o") => "X-X-X-X-",
        (8, "ev") => "-X-X-X-X",
        // Three sections of two: p4 is the middle one again.
        (6, "all") => "XXXXXX",
        (6, "p1") => "XX----",
        (6, "p2") => "--XX--",
        (6, "p3") => "----XX",
        (6, "p4") => "--XX--",
        (6, "p13") => "XX--XX",
        (6, "p24") => "--XX--",
        (6, "e") => "XX--XX",
        (6, "c") => "--XX--",
        (6, "h1") => "XXX---",
        (6, "h2") => "---XXX",
        (6, "o") => "X-X-X-",
        (6, "ev") => "-X-X-X",
        (10, "all") => "XXXXXXXXXX",
        (10, "p1") => "XX--------",
        (10, "p2") => "--XXX-----",
        (10, "p3") => "-----XXX--",
        (10, "p4") => "--------XX",
        (10, "p13") => "XX---XXX--",
        (10, "p24") => "--XXX---XX",
        (10, "e") => "XX------XX",
        (10, "c") => "--XXXXXX--",
        (10, "h1") => "XXXXX-----",
        (10, "h2") => "-----XXXXX",
        (10, "o") => "X-X-X-X-X-",
        (10, "ev") => "-X-X-X-X-X",
        _ => panic!("no mask {region} for {n}"),
    }
}

fn role_char(r: Role) -> char {
    match r {
        Role::Principal => 'P',
        Role::Contraste => 'C',
        Role::Suave => 'S',
        Role::White => 'W',
        Role::Off => '-',
    }
}

/// Each step as `half beats:roles`, e.g. `2:PP--`.
fn rows(seq: &Sequence) -> Vec<String> {
    seq.steps
        .iter()
        .map(|s| {
            let roles: String = s.roles.iter().map(|r| role_char(*r)).collect();
            format!("{}:{roles}", s.len.half_beats())
        })
        .collect()
}

/// Expands a spec row list. Rows are "len base layer..." with len in half beats (a Calm row of
/// 8k half beats is k bars of 8), layers "region=role"; `@k(rows)` repeats rows k times.
fn expected(spec: &str, n: u8) -> Vec<String> {
    let mut out = Vec::new();
    for part in split_top(spec) {
        if let Some(inner) = part.strip_prefix('@') {
            let (k, body) = inner.split_once('(').expect("group");
            let body = body.strip_suffix(')').expect("close");
            let rows = expected(body, n);
            for _ in 0..k.parse::<usize>().expect("k") {
                out.extend(rows.iter().cloned());
            }
            continue;
        }
        let mut tok = part.split_whitespace();
        let half_beats: usize = tok.next().expect("len").parse().expect("len");
        let mut roles: Vec<char> = tok
            .next()
            .expect("base")
            .chars()
            .cycle()
            .take(usize::from(n))
            .collect();
        for layer in tok {
            let (region, role) = layer.split_once('=').expect("layer");
            for (i, m) in mask(n, region).chars().enumerate() {
                if m == 'X' {
                    roles[i] = role.chars().next().expect("role");
                }
            }
        }
        let roles: String = roles.into_iter().collect();
        if half_beats >= 8 {
            out.extend((0..half_beats / 8).map(|_| format!("8:{roles}")));
        } else {
            out.push(format!("{half_beats}:{roles}"));
        }
    }
    out
}

/// Splits on `;` outside parentheses.
fn split_top(spec: &str) -> Vec<&str> {
    let (mut depth, mut start, mut parts) = (0, 0, Vec::new());
    for (i, c) in spec.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ';' if depth == 0 => {
                parts.push(&spec[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&spec[start..]);
    parts
}

fn check(p: Pattern, spec: &str) {
    for n in NS {
        assert_eq!(rows(&build(p, n)), expected(spec, n), "{p:?} n={n}");
    }
}

macro_rules! table {
    ($($name:ident: $p:ident = $spec:expr;)*) => {$(
        #[test]
        fn $name() {
            check(Pattern::$p, $spec);
        }
    )*};
}

table! {
    cross_pairs_matches_table: CrossPairs = "2 - p13=P;2 - p24=P;2 - p13=P;2 - p24=P;2 - p13=C;2 - p24=C;2 - p13=C;2 - p24=C";
    colour_and_dark_matches_table: ColourAndDark = "2 P;2 -;2 C;2 -";
    white_backbeat_matches_table: WhiteBackbeat = "2 - p13=P;2 W;2 - p24=C;2 W";
    outside_in_matches_table: OutsideIn = "2 - e=P;2 - c=P;2 - e=C;2 - c=W";
    halves_blink_matches_table: HalvesBlink = "2 - h1=P;2 - h2=W;2 - h1=P;2 - h2=W";
    cross_pairs_white_backbeat_matches_table: CrossPairsWhiteBackbeat = "2 - p13=P;2 - p24=W;2 - p13=C;2 - p24=W";
    colour_dark_white_matches_table: ColourDarkWhite = "2 P;2 -;2 C;2 W";
    outside_in_with_dark_matches_table: OutsideInWithDark = "2 - e=P;2 -;2 - c=P;2 -;2 - e=C;2 -;2 W;2 -";
    mirror_with_white_matches_table: MirrorWithWhite = "2 - e=P;2 - c=C;2 - e=W;2 -";
    colour_and_dark_two_bars_matches_table: ColourAndDarkTwoBars = "2 P;2 -;2 C;2 -;2 S;2 -;2 W;2 -";
    every_other_beat_matches_table: EveryOtherBeat = "2 - p13=P;2 -;2 - p24=C;2 -";
    halves_white_backbeat_matches_table: HalvesWhiteBackbeat = "2 - h1=P;2 W;2 - h2=C;2 W";
    outside_in_two_bars_matches_table: OutsideInTwoBars = "2 - e=P;2 - c=P;2 - e=C;2 - c=C;2 - e=W;2 -;2 - c=W;2 -";
    cross_pairs_white_turnaround_matches_table: CrossPairsWhiteTurnaround = "2 - p13=P;2 - p24=P;2 - p13=C;2 W";
    ends_centre_all_dark_matches_table: EndsCentreAllDark = "2 - e=P;2 - c=C;2 W;2 -";
    wash_and_soften_matches_table: WashAndSoften = "16 P;16 S";
    ends_white_then_wash_matches_table: EndsWhiteThenWash = "16 - e=W;16 S;16 - c=P;16 S";
    slow_fade_to_dark_matches_table: SlowFadeToDark = "16 P;16 S;16 - h1=S;16 -";
    colour_and_white_matches_table: ColourAndWhite = "16 P;16 C;16 S e=W;16 S";
    centre_to_ends_matches_table: CentreToEnds = "16 S c=W;16 P;16 S e=W;16 S";
    colour_cycle_matches_table: ColourCycle = "16 P;16 C;16 S;16 P c=W";
    fade_and_return_matches_table: FadeAndReturn = "16 P;16 S;16 - c=S;16 P";
    slow_white_breath_matches_table: SlowWhiteBreath = "32 S;16 W;16 P";
    long_wash_matches_table: LongWash = "32 P;32 S";
    white_ends_slowly_matches_table: WhiteEndsSlowly = "32 S e=W;32 P";
    strobe_matches_table: Strobe = "@4(1 W;1 -)";
    converge_matches_table: Converge = "@2(1 - e=W;1 - c=P;1 C;1 -)";
    colour_strobe_matches_table: ColourStrobe = "@4(1 W;1 P)";
    odd_even_strobe_matches_table: OddEvenStrobe = "@4(1 - o=W;1 - ev=W)";
    converge_then_strobe_matches_table: ConvergeThenStrobe = "@2(1 - e=W;1 - c=P;1 C;1 -);@4(1 W;1 -)";
    halves_accelerating_matches_table: HalvesAccelerating = "@2(2 - h1=W h2=P;2 - h1=P h2=W);@4(1 - h1=W h2=P;1 - h1=P h2=W)";
    odd_even_accelerating_matches_table: OddEvenAccelerating = "@2(2 - o=W ev=P;2 - ev=W o=P);@2(1 - o=W ev=P;1 - ev=W o=P);@2(1 W;1 -)";
    strobe_accelerating_matches_table: StrobeAccelerating = "@2(2 W;2 -);@4(1 W;1 -)";
    converge_accelerating_matches_table: ConvergeAccelerating = "2 - e=W;2 - c=P;2 C;2 -;@2(1 - e=W;1 - c=P;1 C;1 -)";
    four_part_chase_accelerating_matches_table: FourPartChaseAccelerating = "2 - p1=P;2 - p2=P;2 - p3=P;2 - p4=P;@2(1 - p1=W;1 - p2=W;1 - p3=W;1 - p4=W)";
}

#[test]
fn literal_examples_from_the_brief() {
    let r = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    assert_eq!(
        rows(&build(Pattern::CrossPairs, 8)),
        r(&[
            "2:PP--PP--",
            "2:--PP--PP",
            "2:PP--PP--",
            "2:--PP--PP",
            "2:CC--CC--",
            "2:--CC--CC",
            "2:CC--CC--",
            "2:--CC--CC"
        ])
    );
    assert_eq!(
        rows(&build(Pattern::EndsCentreAllDark, 10)),
        r(&[
            "2:PP------PP",
            "2:--CCCCCC--",
            "2:WWWWWWWWWW",
            "2:----------"
        ])
    );
    let strobe: Vec<String> = (0..4)
        .flat_map(|_| r(&["1:WWWWWWWW", "1:--------"]))
        .collect();
    assert_eq!(rows(&build(Pattern::Strobe, 8)), strobe);
    let mut slow: Vec<String> = vec!["8:WWSSWW".into(); 4];
    slow.extend(vec!["8:PPPPPP".to_string(); 4]);
    assert_eq!(rows(&build(Pattern::WhiteEndsSlowly, 6)), slow);
}

#[test]
fn rhythm_never_has_half_beat_steps() {
    for p in for_moment(Moment::Rhythm) {
        for n in 1..=16 {
            assert!(
                build(*p, n).steps.iter().all(|s| s.len == StepLen::One),
                "{p:?} n={n}"
            );
        }
    }
}

#[test]
fn calm_never_changes_faster_than_two_bars() {
    for p in for_moment(Moment::Calm) {
        for n in [6u8, 8, 10] {
            let steps = build(*p, n).steps;
            let len = steps.len();
            let Some(start) =
                (0..len).find(|&i| steps[i].roles != steps[(i + len - 1) % len].roles)
            else {
                continue;
            };
            let (mut run, mut prev) = (0u32, &steps[start].roles);
            for k in 0..len {
                let s = &steps[(start + k) % len];
                if &s.roles != prev {
                    assert!(run >= 16, "{p:?} n={n} run={run}");
                    run = 0;
                    prev = &s.roles;
                }
                run += u32::from(s.len.half_beats());
            }
            assert!(run >= 16, "{p:?} n={n} last run={run}");
        }
    }
}

#[test]
fn every_pattern_validates_for_1_to_16_segments() {
    for m in [Moment::Rhythm, Moment::Calm, Moment::BuildUp] {
        for p in for_moment(m) {
            for n in 1..=16u8 {
                let seq = build(*p, n);
                assert!(seq.validate(n).is_ok(), "{p:?} n={n}");
                let half_beats: u32 = seq
                    .steps
                    .iter()
                    .map(|s| u32::from(s.len.half_beats()))
                    .sum();
                assert_eq!(half_beats % 8, 0, "{p:?} n={n}");
                assert!(
                    half_beats <= 16 * 8 && seq.steps.len() <= 128,
                    "{p:?} n={n}"
                );
                assert_eq!(seq.moment, p.moment());
                assert_eq!(seq.moment, m);
                assert!(seq.fade);
                assert_eq!(seq.name, p.label());
                assert_eq!(seq, build(*p, n));
            }
        }
    }
}

#[test]
fn for_moment_lists_35_in_table_order() {
    let len = |m| for_moment(m).len();
    assert_eq!(
        (len(Moment::Rhythm), len(Moment::Calm), len(Moment::BuildUp)),
        (15, 10, 10)
    );
    assert_eq!(for_moment(Moment::Rhythm)[0], Pattern::CrossPairs);
    assert_eq!(for_moment(Moment::Calm)[9], Pattern::WhiteEndsSlowly);
    assert_eq!(for_moment(Moment::BuildUp)[4], Pattern::ConvergeThenStrobe);
    assert_eq!(
        serde_json::to_string(&Pattern::ConvergeThenStrobe).expect("json"),
        r#""convergeThenStrobe""#
    );
}

#[test]
fn labels_are_the_spec_names() {
    assert_eq!(
        Pattern::CrossPairsWhiteBackbeat.label(),
        "Cross pairs + white backbeat"
    );
    assert_eq!(
        Pattern::OddEvenAccelerating.label(),
        "Odd / even, accelerating"
    );
    assert_eq!(
        Pattern::FourPartChaseAccelerating.label(),
        "Four-part chase, accelerating"
    );
}

#[test]
fn each_table_holds_only_patterns_of_its_moment() {
    use light_core::detect::Moment::{BuildUp, Calm, Rhythm};
    for m in [Rhythm, Calm, BuildUp] {
        for p in light_core::patterns::for_moment(m) {
            assert_eq!(p.moment(), m, "{p:?}");
        }
    }
}
