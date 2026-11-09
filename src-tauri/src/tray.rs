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

macro_rules! moment_images {
    ($($kind:literal),*) => {
        // The committed @2x images (`tools/tray-frames`); @1x files exist for non-retina use.
        static IMAGES: &[(&str, &[u8])] = &[
            entry!("tray-stopped"), entry!("tray-stopped-lost"),
            $(
                entry!(concat!("tray-", $kind)), entry!(concat!("tray-", $kind, "-dim")),
                entry!(concat!("tray-", $kind, "-forced")), entry!(concat!("tray-", $kind, "-forced-dim")),
                entry!(concat!("tray-", $kind, "-lost")), entry!(concat!("tray-", $kind, "-dim-lost")),
                entry!(concat!("tray-", $kind, "-forced-lost")), entry!(concat!("tray-", $kind, "-forced-dim-lost")),
            )*
        ];
    };
}
moment_images!("rhythm", "calm", "buildup");

fn image(name: &str) -> Option<Image<'static>> {
    let (_, bytes) = IMAGES.iter().find(|(n, _)| *n == name)?;
    Image::from_bytes(bytes).ok()
}

/// Paints `view` on the tray item, touching only what differs from `prev`.
pub fn apply(app: &AppHandle, prev: Option<&TrayView>, view: &TrayView) {
    let Some(main) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if prev.is_none_or(|p| p.image != view.image) {
        // Lost images are coloured (grey + red dot): not template.
        let _ = main.set_icon_as_template(!view.image.lost);
        let _ = main.set_icon(image(&view.image.name()));
    }
    if prev.is_none_or(|p| p.title != view.title) {
        let _ = main.set_title(Some(view.title.as_str()));
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

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(image("tray-stopped-lost").ok_or("tray image missing")?)
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

/// Menu bar state in, what the single tray item shows out (spec §7.1). Pure.
use light_core::detect::Moment;

#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // the specified input record
pub struct TrayInput {
    pub midi_ok: bool,
    pub strip_ok: bool,
    pub playing: bool,
    pub bpm: Option<f32>,
    pub last_beat_us: Option<u64>,
    pub moment: Moment,
    pub forced: bool,
}

/// Which bundled image to show: the LED glyph plus the moment icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct TrayImage {
    /// `None` when stopped: the glyph alone.
    pub moment: Option<Moment>,
    /// Full icon on the beat; dimmed between beats.
    pub lit: bool,
    /// Forced moment: "F" badge.
    pub forced: bool,
    /// No MIDI or no strip: red dot on the glyph.
    pub lost: bool,
}

impl TrayImage {
    /// `tray-<moment|stopped>[-forced][-dim][-lost]`
    #[must_use]
    pub fn name(self) -> String {
        let kind = match self.moment {
            None => return format!("tray-stopped{}", if self.lost { "-lost" } else { "" }),
            Some(Moment::Rhythm) => "rhythm",
            Some(Moment::Calm) => "calm",
            Some(Moment::BuildUp) => "buildup",
        };
        format!(
            "tray-{kind}{}{}{}",
            if self.forced { "-forced" } else { "" },
            if self.lit { "" } else { "-dim" },
            if self.lost { "-lost" } else { "" }
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayView {
    pub image: TrayImage,
    pub title: String,
}

/// How long the moment icon stays bright after a beat.
const BLINK_US: u64 = 120_000;

#[must_use]
pub fn tray_view(s: &TrayInput, now_us: u64) -> TrayView {
    let image = TrayImage {
        moment: s.playing.then_some(s.moment),
        lit: s.playing
            && s.last_beat_us
                .is_some_and(|t| now_us.saturating_sub(t) < BLINK_US),
        forced: s.playing && s.forced,
        lost: !(s.midi_ok && s.strip_ok),
    };
    // U+2007 (figure space) keeps the width of "100.0" so the menu bar does not jitter.
    let title = s.bpm.map_or_else(
        || "\u{2014}".to_string(),
        |b| format!("{:\u{2007}>5}", format!("{b:.1}")),
    );
    TrayView { image, title }
}

/// The next instant the view changes by itself (the blink going dim); none when stopped.
#[must_use]
pub fn tray_deadline(s: &TrayInput, now_us: u64) -> Option<u64> {
    if !s.playing {
        return None;
    }
    s.last_beat_us.map(|t| t + BLINK_US).filter(|&d| d > now_us)
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
            moment: Moment::Rhythm,
            forced: false,
        }
    }

    fn playing() -> TrayInput {
        TrayInput {
            playing: true,
            last_beat_us: Some(1_000_000),
            ..input()
        }
    }

    #[test]
    fn single_tray_item_only() {
        let src = include_str!("tray.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap()];
        assert!(!code.contains("WAVE_ID") && !code.contains("FRAME_US"));
        assert_eq!(code.matches("TrayIconBuilder::with_id").count(), 1);
    }

    #[test]
    fn moment_icon_blinks_on_beat() {
        let i = playing();
        assert_eq!(tray_view(&i, 1_119_999).image.name(), "tray-rhythm");
        assert_eq!(tray_view(&i, 1_120_000).image.name(), "tray-rhythm-dim");
        assert_eq!(tray_deadline(&i, 1_100_000), Some(1_120_000));
        // between beats nothing changes by itself until the next beat event
        assert_eq!(tray_deadline(&i, 1_130_000), None);
        let calm = TrayInput {
            moment: Moment::Calm,
            ..i.clone()
        };
        assert_eq!(tray_view(&calm, 1_000_000).image.name(), "tray-calm");
        let up = TrayInput {
            moment: Moment::BuildUp,
            ..i
        };
        assert_eq!(tray_view(&up, 1_200_000).image.name(), "tray-buildup-dim");
    }

    #[test]
    fn forced_shows_badge() {
        let i = TrayInput {
            forced: true,
            ..playing()
        };
        assert_eq!(tray_view(&i, 1_000_000).image.name(), "tray-rhythm-forced");
        assert_eq!(
            tray_view(&i, 1_500_000).image.name(),
            "tray-rhythm-forced-dim"
        );
    }

    #[test]
    fn stopped_shows_dash() {
        let v = tray_view(&input(), 5);
        assert_eq!(v.image.name(), "tray-stopped");
        assert_eq!(v.title, "\u{2014}");
        // forced while stopped shows no moment, so no badge either
        let f = TrayInput {
            forced: true,
            ..input()
        };
        assert_eq!(tray_view(&f, 5).image.name(), "tray-stopped");
    }

    #[test]
    fn lost_strip_shows_red_dot() {
        for (m, s) in [(false, true), (true, false), (false, false)] {
            let i = TrayInput {
                midi_ok: m,
                strip_ok: s,
                ..input()
            };
            assert_eq!(tray_view(&i, 0).image.name(), "tray-stopped-lost");
        }
        let i = TrayInput {
            strip_ok: false,
            forced: true,
            ..playing()
        };
        assert_eq!(
            tray_view(&i, 1_500_000).image.name(),
            "tray-rhythm-forced-dim-lost"
        );
    }

    #[test]
    fn no_deadline_when_stopped() {
        let i = TrayInput {
            last_beat_us: Some(1_000_000),
            ..input()
        };
        assert_eq!(tray_deadline(&i, 1_050_000), None);
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
    fn every_image_name_is_bundled() {
        for moment in [
            None,
            Some(Moment::Rhythm),
            Some(Moment::Calm),
            Some(Moment::BuildUp),
        ] {
            for (forced, lit, lost) in [
                (false, true, false),
                (true, false, true),
                (false, false, false),
            ] {
                let name = TrayImage {
                    moment,
                    lit,
                    forced: forced && moment.is_some(),
                    lost,
                }
                .name();
                assert!(IMAGES.iter().any(|(n, _)| *n == name), "{name} not bundled");
            }
        }
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
