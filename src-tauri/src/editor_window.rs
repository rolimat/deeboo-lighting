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
            .build()
            .map(|_| ())
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
                "allow-set-live-set",
                "allow-pattern-sequence",
                "allow-detection-watch",
                "allow-set-detection-thresholds",
                "core:event:allow-listen",
                "core:event:allow-unlisten"
            ]
        );
    }
}
