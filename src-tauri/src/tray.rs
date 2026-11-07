//! Tray icon and popover toggling.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

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

fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window(POPOVER) else {
        log::error!("popover window is missing");
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
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
}

fn on_popover_blur(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(POPOVER) {
        let _ = window.hide();
    }
    *app.state::<BlurHide>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
}

pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.manage(BlurHide::default());

    // Placeholder template icon (monochrome, tinted by macOS); the final icon is a later plan.
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(tray.app_handle());
            }
        })
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

#[cfg(test)]
mod tests {
    use super::*;

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
