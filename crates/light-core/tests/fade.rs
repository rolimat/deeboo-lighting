use light_core::fade::{frames, mix};
use light_core::paint::Hsv;

const RED: Hsv = Hsv {
    h: 0,
    s: 1000,
    v: 1000,
};
const BLUE: Hsv = Hsv {
    h: 240,
    s: 1000,
    v: 1000,
};

#[test]
fn mix_ends_exact_and_starts_near() {
    assert_eq!(mix(Some(RED), Some(BLUE), 4, 4), Some(BLUE));
    assert_eq!(
        mix(None, Some(RED), 2, 4),
        Some(Hsv {
            h: 0,
            s: 1000,
            v: 500
        })
    );
    assert_eq!(mix(Some(RED), None, 4, 4), None);
}

#[test]
fn mix_red_to_blue_never_passes_green() {
    for k in 1..=4 {
        let h = mix(Some(RED), Some(BLUE), k, 4).unwrap().h;
        assert!(h == 0 || (240..=360).contains(&h), "{k}: {h}");
    }
}

#[test]
fn frames_rules() {
    assert_eq!(frames(8), 4);
    assert_eq!(frames(16), 4);
    assert_eq!(frames(4), 2);
    assert_eq!(frames(2), 1);
    assert_eq!(frames(1), 1);
}
