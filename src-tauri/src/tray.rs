//! Tray icon and popover toggling.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::commands::AppState;

const POPOVER: &str = "popover";
const TRAY_ID: &str = "main";
/// Created first, so macOS places it to the right of `main`.
const WAVE_ID: &str = "wave";
/// A tray click this soon after a blur hid the popover is the same click that caused the blur.
const BLUR_CLICK_WINDOW: Duration = Duration::from_millis(250);

/// When the popover was last hidden because it lost focus.
#[derive(Default)]
struct BlurHide(Mutex<Option<Instant>>);

fn hidden_by_blur_just_now(hidden_at: Option<Instant>, now: Instant) -> bool {
    hidden_at.is_some_and(|t| now.saturating_duration_since(t) < BLUR_CLICK_WINDOW)
}

/// Tells the engine thread whether the popover is on screen (it publishes status only then).
fn set_visible(app: &AppHandle, visible: bool) {
    app.state::<AppState>().set_popover_visible(visible);
}

fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window(POPOVER) else {
        log::error!("popover window is missing");
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        set_visible(app, false);
        return;
    }
    let hidden_at = *app
        .state::<BlurHide>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if hidden_by_blur_just_now(hidden_at, Instant::now()) {
        return;
    }
    if let Err(e) = window.move_window(Position::TrayBottomCenter) {
        log::error!("cannot position popover: {e}");
    }
    let _ = window.show();
    let _ = window.set_focus();
    set_visible(app, true);
}

fn on_popover_blur(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(POPOVER) {
        let _ = window.hide();
    }
    set_visible(app, false);
    *app.state::<BlurHide>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
}

macro_rules! entry {
    ($name:expr) => {
        (
            $name,
            include_bytes!(concat!("../icons/tray/", $name, "@2x.png")).as_slice(),
        )
    };
}

macro_rules! wave_entries {
    ($($kind:literal),*) => {
        // The committed @2x frames (`tools/tray-frames`); @1x files exist for non-retina use.
        static IMAGES: &[(&str, &[u8])] = &[
            entry!("dot-red"), entry!("dot-green-on"), entry!("dot-green-dim"),
            $(
                entry!(concat!($kind, "-0")), entry!(concat!($kind, "-1")),
                entry!(concat!($kind, "-2")), entry!(concat!($kind, "-3")),
                entry!(concat!($kind, "-4")), entry!(concat!($kind, "-5")),
                entry!(concat!($kind, "-6")), entry!(concat!($kind, "-7")),
                entry!(concat!($kind, "-0-forced")), entry!(concat!($kind, "-1-forced")),
                entry!(concat!($kind, "-2-forced")), entry!(concat!($kind, "-3-forced")),
                entry!(concat!($kind, "-4-forced")), entry!(concat!($kind, "-5-forced")),
                entry!(concat!($kind, "-6-forced")), entry!(concat!($kind, "-7-forced")),
            )*
        ];
    };
}
wave_entries!("rhythm", "calm", "buildup");

fn image(name: &str) -> Option<Image<'static>> {
    let (_, bytes) = IMAGES.iter().find(|(n, _)| *n == name)?;
    Image::from_bytes(bytes).ok()
}

fn dot_name(d: Dot) -> &'static str {
    match d {
        Dot::Red => "dot-red",
        Dot::GreenOn => "dot-green-on",
        Dot::GreenDim => "dot-green-dim",
    }
}

fn wave_name((moment, frame, forced): (Moment, u8, bool)) -> String {
    let kind = match moment {
        Moment::Rhythm => "rhythm",
        Moment::Calm => "calm",
        Moment::BuildUp => "buildup",
    };
    format!("{kind}-{frame}{}", if forced { "-forced" } else { "" })
}

/// Paints `view` on the two icons, touching only what differs from `prev`.
pub fn apply(app: &AppHandle, prev: Option<&TrayView>, view: &TrayView) {
    if let Some(main) = app.tray_by_id(TRAY_ID) {
        if prev.is_none_or(|p| p.dot != view.dot) {
            let _ = main.set_icon(image(dot_name(view.dot)));
        }
        if prev.is_none_or(|p| p.title != view.title) {
            let _ = main.set_title(Some(view.title.as_str()));
        }
    }
    let old = prev.and_then(|p| p.wave);
    if old == view.wave {
        return;
    }
    let Some(wave) = app.tray_by_id(WAVE_ID) else {
        return;
    };
    match view.wave {
        Some(w) => {
            let _ = wave.set_icon(image(&wave_name(w)));
            if old.is_none() {
                let _ = wave.set_visible(true);
            }
        }
        None => {
            let _ = wave.set_visible(false);
        }
    }
}

fn on_click(tray: &TrayIcon, event: &TrayIconEvent) {
    tauri_plugin_positioner::on_tray_event(tray.app_handle(), event);
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        toggle_popover(tray.app_handle());
    }
}

pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.manage(BlurHide::default());

    // `wave` first: macOS puts the newest item to the left. Hidden until the clock plays.
    let wave = TrayIconBuilder::with_id(WAVE_ID)
        .icon(image("rhythm-0").ok_or("tray frame missing")?)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| on_click(tray, &event))
        .build(app)?;
    wave.set_visible(false)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(image("dot-red").ok_or("tray frame missing")?)
        .icon_as_template(false)
        .title("\u{2014}")
        .on_tray_icon_event(|tray, event| on_click(tray, &event))
        .build(app)?;

    if let Some(window) = app.get_webview_window(POPOVER) {
        let handle = app.handle().clone();
        window.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                on_popover_blur(&handle);
            }
        });
    }
    Ok(())
}

/// Menu bar state in, what the two tray icons show out (spec §7.1). Pure.
use light_core::detect::Moment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Red,
    GreenOn,
    GreenDim,
}

#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // the specified input record
pub struct TrayInput {
    pub midi_ok: bool,
    pub strip_ok: bool,
    pub playing: bool,
    pub bpm: Option<f32>,
    pub last_beat_us: Option<u64>,
    pub play_start_us: Option<u64>,
    pub moment: Moment,
    pub forced: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayView {
    pub dot: Dot,
    pub title: String,
    /// Moment, frame 0..8, forced.
    pub wave: Option<(Moment, u8, bool)>,
}

/// How long the dot stays bright after a beat.
const BLINK_US: u64 = 120_000;
/// 12 wave frames per second.
const FRAME_US: u64 = 83_333;
const FRAMES: u64 = 8;

#[must_use]
pub fn tray_view(s: &TrayInput, now_us: u64) -> TrayView {
    let dot = if !(s.midi_ok && s.strip_ok) {
        Dot::Red
    } else if s.playing
        && s.last_beat_us
            .is_none_or(|t| now_us.saturating_sub(t) >= BLINK_US)
    {
        Dot::GreenDim
    } else {
        Dot::GreenOn
    };
    // U+2007 (figure space) keeps the width of "100.0" so the menu bar does not jitter.
    let title = s.bpm.map_or_else(
        || "\u{2014}".to_string(),
        |b| format!("{:\u{2007}>5}", format!("{b:.1}")),
    );
    #[allow(clippy::cast_possible_truncation)] // % FRAMES < 8
    let wave = s.playing.then(|| {
        let elapsed = now_us.saturating_sub(s.play_start_us.unwrap_or(now_us));
        (s.moment, (elapsed / FRAME_US % FRAMES) as u8, s.forced)
    });
    TrayView { dot, title, wave }
}

/// The next instant the view changes by itself; only while playing (no polling when stopped).
#[must_use]
pub fn tray_deadline(s: &TrayInput, now_us: u64) -> Option<u64> {
    if !s.playing {
        return None;
    }
    let start = s.play_start_us.unwrap_or(now_us);
    let next_frame = start + (now_us.saturating_sub(start) / FRAME_US + 1) * FRAME_US;
    let blink_off = s.last_beat_us.map(|t| t + BLINK_US).filter(|&d| d > now_us);
    Some(blink_off.map_or(next_frame, |d| d.min(next_frame)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> TrayInput {
        TrayInput {
            midi_ok: true,
            strip_ok: true,
            playing: false,
            bpm: None,
            last_beat_us: None,
            play_start_us: None,
            moment: Moment::Rhythm,
            forced: false,
        }
    }

    #[test]
    fn red_without_midi_or_strip() {
        for (m, s) in [(false, true), (true, false), (false, false)] {
            let i = TrayInput {
                midi_ok: m,
                strip_ok: s,
                ..input()
            };
            assert_eq!(tray_view(&i, 0).dot, Dot::Red);
        }
    }

    #[test]
    fn green_steady_when_connected_and_stopped() {
        assert_eq!(tray_view(&input(), 5).dot, Dot::GreenOn);
    }

    #[test]
    fn blinks_on_each_beat_while_playing() {
        let i = TrayInput {
            playing: true,
            last_beat_us: Some(1_000_000),
            play_start_us: Some(0),
            ..input()
        };
        assert_eq!(tray_view(&i, 1_119_999).dot, Dot::GreenOn);
        assert_eq!(tray_view(&i, 1_120_000).dot, Dot::GreenDim);
        assert_eq!(tray_deadline(&i, 1_100_000), Some(1_120_000));
    }

    #[test]
    fn bpm_title_padding_and_dash() {
        let t = |bpm| tray_view(&TrayInput { bpm, ..input() }, 0).title;
        assert_eq!(t(Some(125.0)), "125.0");
        assert_eq!(t(Some(9.5)), "\u{2007}\u{2007}9.5");
        assert_eq!(t(Some(99.94)), "\u{2007}99.9");
        assert_eq!(t(Some(100.0)), "100.0");
        assert_eq!(t(None), "\u{2014}");
    }

    #[test]
    fn wave_frame_advances_at_12_fps_only_while_playing() {
        let mut i = TrayInput {
            play_start_us: Some(1_000_000),
            ..input()
        };
        assert_eq!(tray_view(&i, 2_000_000).wave, None);
        assert_eq!(tray_deadline(&i, 2_000_000), None);
        i.playing = true;
        let f = |now| tray_view(&i, now).wave.map(|w| w.1);
        assert_eq!(f(1_000_000), Some(0));
        assert_eq!(f(1_083_332), Some(0));
        assert_eq!(f(1_083_333), Some(1));
        assert_eq!(f(1_000_000 + 8 * 83_333), Some(0));
        assert_eq!(tray_deadline(&i, 1_010_000), Some(1_083_333));
    }

    #[test]
    fn forced_moment_uses_forced_frames() {
        let i = TrayInput {
            playing: true,
            play_start_us: Some(0),
            moment: Moment::BuildUp,
            forced: true,
            ..input()
        };
        assert_eq!(tray_view(&i, 0).wave, Some((Moment::BuildUp, 0, true)));
    }

    #[test]
    fn click_right_after_blur_hide_is_ignored() {
        let t0 = Instant::now();
        assert!(hidden_by_blur_just_now(
            Some(t0),
            t0 + Duration::from_millis(100)
        ));
        assert!(!hidden_by_blur_just_now(
            Some(t0),
            t0 + Duration::from_millis(300)
        ));
        assert!(!hidden_by_blur_just_now(None, t0));
    }
}
