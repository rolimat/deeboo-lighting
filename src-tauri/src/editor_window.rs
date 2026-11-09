//! The editor window: created on open, destroyed on close (no hidden window kept alive).

use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const EDITOR: &str = "editor";

/// Editor sections; anything else is rejected by serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Section {
    Strip,
}

/// The window manager behind a trait so "open twice" is testable without a display.
trait Windows {
    /// Focuses the editor if it exists; false when there is none.
    fn focus(&self) -> bool;
    fn create(&self) -> Result<(), String>;
}

fn open(w: &impl Windows) -> Result<(), String> {
    if w.focus() {
        return Ok(());
    }
    w.create()
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

/// Opens (or focuses) the editor; the window shows Strip by default, so the section is not sent.
pub fn open_at_launch(app: &AppHandle) {
    let _ = open(app);
}

impl Windows for AppHandle {
    fn focus(&self) -> bool {
        self.get_webview_window(EDITOR)
            .is_some_and(|w| w.unminimize().is_ok() && w.set_focus().is_ok())
    }

    fn create(&self) -> Result<(), String> {
        WebviewWindowBuilder::new(self, EDITOR, WebviewUrl::App("editor.html".into()))
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
pub async fn open_editor(app: AppHandle, _section: Section) -> Result<(), String> {
    open(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    struct Fake {
        exists: Cell<bool>,
        created: Cell<u32>,
        focused: Cell<u32>,
    }

    impl Windows for Fake {
        fn focus(&self) -> bool {
            if self.exists.get() {
                self.focused.set(self.focused.get() + 1);
            }
            self.exists.get()
        }
        fn create(&self) -> Result<(), String> {
            self.created.set(self.created.get() + 1);
            self.exists.set(true);
            Ok(())
        }
    }

    #[test]
    fn opening_twice_focuses_existing_window() {
        let w = Fake::default();
        assert!(open(&w).is_ok());
        assert!(open(&w).is_ok());
        assert_eq!((w.created.get(), w.focused.get()), (1, 1));
    }

    #[test]
    fn first_launch_without_strip_opens_strip_section() {
        assert_eq!(first_launch_section(false), Some(Section::Strip));
        assert_eq!(first_launch_section(true), None);
    }

    #[test]
    fn section_parses_and_rejects_unknown() {
        assert_eq!(
            serde_json::from_str::<Section>(r#""strip""#).ok(),
            Some(Section::Strip)
        );
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
        // The editor reads the strip status and the library, drives the wizard and listens for events; no window or popover powers.
        assert_eq!(
            perms,
            [
                "allow-get-status",
                "allow-get-settings",
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
                "allow-pattern-sequence",
                "allow-detection-watch",
                "allow-set-detection-thresholds",
                "core:event:allow-listen",
                "core:event:allow-unlisten"
            ]
        );
    }
}
