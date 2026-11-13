//! The editor window: created on open, destroyed on close (no hidden window kept alive).

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const EDITOR: &str = "editor";

/// Editor sections; anything else is rejected by serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Section {
    Sets,
    Sequences,
    Palettes,
    Strip,
    Detection,
    Setup,
}

/// Event sent to an already open editor so it switches tab.
const SECTION_EVENT: &str = "editor-section";

/// The editor page for a section; the UI reads `?section=` on load.
fn editor_url(section: Section) -> String {
    let name = match section {
        Section::Sets => "sets",
        Section::Sequences => "sequences",
        Section::Palettes => "palettes",
        Section::Strip => "strip",
        Section::Detection => "detection",
        Section::Setup => "setup",
    };
    format!("editor.html?section={name}")
}

/// Only the bundled app (`tauri://localhost`) may load in the editor; in debug builds also the Vite dev server.
/// A dropped file or link, a stray `<a>` or a redirect to anything else is refused.
fn navigation_allowed(url: &tauri::Url) -> bool {
    let host = url.host_str();
    let app = url.scheme() == "tauri" && host == Some("localhost");
    let dev = cfg!(debug_assertions)
        && url.scheme() == "http"
        && host == Some("localhost")
        && url.port() == Some(5173);
    app || dev
}

/// Plugin that applies [`navigation_allowed`] to every webview (editor and the config-defined popover).
#[must_use]
pub fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation")
        .on_navigation(|_webview, url| {
            let ok = navigation_allowed(url);
            if !ok {
                log::debug!("navigation refused: {}://", url.scheme());
            }
            ok
        })
        .build()
}

/// The window manager behind a trait so "open twice" is testable without a display.
trait Windows {
    /// Focuses the editor on `section` if it exists; false when there is none.
    fn focus(&self, section: Section) -> bool;
    fn create(&self, section: Section) -> Result<(), String>;
}

fn open(w: &impl Windows, section: Section) -> Result<(), String> {
    if w.focus(section) {
        return Ok(());
    }
    w.create(section)
}

/// Where the editor opens by itself at launch: on Strip while no strip is saved.
#[must_use]
pub const fn first_launch_section(has_strip: bool) -> Option<Section> {
    if has_strip {
        None
    } else {
        Some(Section::Strip)
    }
}

/// Opens (or focuses) the editor on Strip.
pub fn open_at_launch(app: &AppHandle) {
    let _ = open(app, Section::Strip);
}

/// Dock icon on or off (macOS activation policy); the menu bar item stays either way.
pub fn apply_dock(app: &AppHandle, editor_open: bool) {
    let mode = app
        .state::<crate::commands::AppState>()
        .settings()
        .show_in_dock;
    let visible = crate::settings::dock_visible(mode, editor_open);
    #[cfg(target_os = "macos")]
    {
        let policy = if visible {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        if let Err(e) = app.set_activation_policy(policy) {
            log::warn!("dock policy not applied: {e}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = visible;
}

impl Windows for AppHandle {
    fn focus(&self, section: Section) -> bool {
        let shown = self
            .get_webview_window(EDITOR)
            .is_some_and(|w| w.unminimize().is_ok() && w.set_focus().is_ok());
        if shown && let Err(e) = self.emit_to(EDITOR, SECTION_EVENT, section) {
            log::warn!("editor section not delivered: {e}");
        }
        shown
    }

    fn create(&self, section: Section) -> Result<(), String> {
        WebviewWindowBuilder::new(self, EDITOR, WebviewUrl::App(editor_url(section).into()))
            .title("Deeboo Lighting")
            .inner_size(720.0, 560.0)
            .resizable(true)
            .focused(true)
            // The native file-drop handler swallows `WKWebView` drops, so HTML5 drag and drop never fires.
            .disable_drag_drop_handler()
            .build()
            .map(|_| apply_dock(self, true))
            .map_err(|e| {
                log::warn!("editor window not created: {e}");
                "editor unavailable".to_string()
            })
    }
}

/// Opens the editor on `section`, or focuses it if already open.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // tauri passes the handle by value
pub async fn open_editor(app: AppHandle, section: Section) -> Result<(), String> {
    open(&app, section)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct Fake {
        exists: Cell<bool>,
        created: RefCell<Vec<Section>>,
        focused: RefCell<Vec<Section>>,
    }

    impl Windows for Fake {
        fn focus(&self, section: Section) -> bool {
            if self.exists.get() {
                self.focused.borrow_mut().push(section);
            }
            self.exists.get()
        }
        fn create(&self, section: Section) -> Result<(), String> {
            self.created.borrow_mut().push(section);
            self.exists.set(true);
            Ok(())
        }
    }

    #[test]
    fn opening_twice_creates_once_then_tells_the_open_window_the_section() {
        let w = Fake::default();
        assert!(open(&w, Section::Sets).is_ok());
        assert!(open(&w, Section::Setup).is_ok());
        assert_eq!(*w.created.borrow(), [Section::Sets]);
        assert_eq!(*w.focused.borrow(), [Section::Setup]);
    }

    #[test]
    fn first_launch_without_strip_opens_strip_section() {
        assert_eq!(first_launch_section(false), Some(Section::Strip));
        assert_eq!(first_launch_section(true), None);
    }

    #[test]
    fn all_six_sections_parse_and_map_to_the_window_url() {
        for (name, section) in [
            ("sets", Section::Sets),
            ("sequences", Section::Sequences),
            ("palettes", Section::Palettes),
            ("strip", Section::Strip),
            ("detection", Section::Detection),
            ("setup", Section::Setup),
        ] {
            let json = format!("\"{name}\"");
            assert_eq!(serde_json::from_str::<Section>(&json).ok(), Some(section));
            assert_eq!(editor_url(section), format!("editor.html?section={name}"));
        }
        assert!(serde_json::from_str::<Section>(r#""secrets""#).is_err());
    }

    #[test]
    fn editor_capability_lists_only_editor_commands() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/editor.json")).unwrap_or_default();
        assert_eq!(cap["windows"], serde_json::json!(["editor"]));
        let perms: Vec<&str> = cap["permissions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .collect();
        // The editor reads the strip status and the library, edits the settings (Setup tab), drives the wizard and listens for events; no window or popover powers.
        assert_eq!(
            perms,
            [
                "allow-get-status",
                "allow-get-settings",
                "allow-save-settings",
                "allow-list-midi-sources",
                "allow-list-audio-inputs",
                "allow-get-library",
                "allow-save-library",
                "allow-wizard-start",
                "allow-wizard-choose",
                "allow-wizard-retry",
                "allow-wizard-cancel",
                "allow-strip-identify",
                "allow-identify-segment",
                "allow-wizard-save",
                "allow-set-wired-segments",
                "allow-message-counts",
                "allow-preview-generated",
                "allow-preview-palette",
                "allow-preview-sequence",
                "allow-calibrate-latency",
                "allow-quit",
                "allow-set-live-set",
                "allow-pattern-sequence",
                "allow-detection-watch",
                "allow-set-detection-thresholds",
                "core:event:allow-listen",
                "core:event:allow-unlisten"
            ]
        );
    }

    #[test]
    fn only_the_bundled_app_may_load_in_the_editor() {
        let url = |s: &str| tauri::Url::parse(s).unwrap();
        assert!(navigation_allowed(&url(
            "tauri://localhost/editor.html?section=sets"
        )));
        for refused in [
            "file:///Users/me/Downloads/x.html",
            "https://example.com/",
            "http://example.com/",
            "tauri://evil.example/",
            "http://localhost:8080/",
            "https://localhost:5173/",
            "data:text/html,hi",
        ] {
            assert!(!navigation_allowed(&url(refused)), "{refused}");
        }
        assert_eq!(
            navigation_allowed(&url("http://localhost:5173/editor.html")),
            cfg!(debug_assertions)
        );
    }

    #[test]
    fn the_navigation_guard_plugin_is_registered_for_every_webview() {
        let lib = include_str!("lib.rs");
        assert!(lib.contains(".plugin(editor_window::navigation_guard())"));
    }

    /// The editor uses HTML5 drag and drop (Sets, Palettes, Sequences); on macOS the native
    /// file-drop handler takes every drop away from `WKWebView`, so it must stay off.
    #[test]
    fn editor_window_keeps_the_native_drop_handler_off() {
        let src = include_str!("editor_window.rs");
        let builder = src
            .split("WebviewWindowBuilder::new")
            .nth(1)
            .unwrap_or_default();
        let builder = builder.split(".build()").next().unwrap_or_default();
        assert!(builder.contains(".disable_drag_drop_handler()"));
    }
}
