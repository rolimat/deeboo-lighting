//! IPC commands. Every DTO here is mirrored in `ui/src/api.ts`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StripStatus {
    NotConfigured,
    #[allow(dead_code)] // set once the strip connection exists
    Connecting,
    #[allow(dead_code)]
    Connected {
        name: String,
        segments: u8,
    },
    #[allow(dead_code)]
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub strip: StripStatus,
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn get_status() -> Status {
    Status {
        strip: StripStatus::NotConfigured,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serialises_as_the_ui_expects() {
        let json = |s: &Status| serde_json::to_string(s).unwrap_or_default();
        assert_eq!(json(&get_status()), r#"{"strip":{"kind":"notConfigured"}}"#);
        let connected = Status {
            strip: StripStatus::Connected {
                name: "dj lights".into(),
                segments: 10,
            },
        };
        assert_eq!(
            json(&connected),
            r#"{"strip":{"kind":"connected","name":"dj lights","segments":10}}"#
        );
    }
}
