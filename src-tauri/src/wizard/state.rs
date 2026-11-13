//! Wizard states as the UI sees them, and the pure transitions for UI-driven events.
//! Nothing here can hold a secret: the only cloud-derived value is the QR matrix.

use crate::qr::QrMatrix;
use serde::Serialize;

/// Wizard step a failure belongs to.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Login,
    Devices,
    Search,
    Segments,
    Save,
}

/// A device as listed to the UI. No key, ever.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDto {
    pub id: String,
    pub name: String,
    pub product: String,
    pub online: bool,
    /// Category `dd` (light strip).
    pub supported: bool,
    /// Why the device can't be chosen, when the cause is its data (no usable local key).
    pub reason: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WizardState {
    Idle,
    WaitingScan {
        qr: QrMatrix,
        expires_at_us: u64,
    },
    ChooseStrip {
        devices: Vec<DeviceDto>,
    },
    Searching {
        device: DeviceDto,
        since_us: u64,
        help: bool,
    },
    Listening {
        device: DeviceDto,
        since_us: u64,
    },
    Identify {
        device: DeviceDto,
        segments: u8,
    },
    Saved {
        device: DeviceDto,
        segments: u8,
    },
    Failed {
        step: Step,
        message: String,
    },
}

/// UI-driven events.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "specification of the transitions; the session enforces them directly"
    )
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Choose(String),
    Cancel,
    Retry,
}

/// Next state for a UI event at `now_us`. Events that make no sense in a state leave it unchanged.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "specification of the transitions; the session enforces them directly"
    )
)]
#[must_use]
pub fn next(state: WizardState, event: &Event, now_us: u64) -> WizardState {
    match (state, event) {
        (_, Event::Cancel) | (WizardState::Failed { .. }, Event::Retry) => WizardState::Idle,
        (WizardState::ChooseStrip { devices }, Event::Choose(id)) => {
            match devices.iter().find(|d| d.id == *id && d.supported) {
                Some(device) => WizardState::Searching {
                    device: device.clone(),
                    since_us: now_us,
                    help: false,
                },
                None => WizardState::ChooseStrip { devices },
            }
        }
        (state, _) => state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, supported: bool) -> DeviceDto {
        DeviceDto {
            id: id.into(),
            name: "Strip".into(),
            product: "P".into(),
            online: true,
            supported,
            reason: None,
        }
    }

    fn all_states() -> Vec<WizardState> {
        let d = dev("a", true);
        vec![
            WizardState::Idle,
            WizardState::WaitingScan {
                qr: QrMatrix {
                    size: 1,
                    modules: vec![true],
                },
                expires_at_us: 9,
            },
            WizardState::ChooseStrip {
                devices: vec![d.clone(), dev("b", false)],
            },
            WizardState::Searching {
                device: d.clone(),
                since_us: 1,
                help: true,
            },
            WizardState::Listening {
                device: d.clone(),
                since_us: 1,
            },
            WizardState::Identify {
                device: d.clone(),
                segments: 3,
            },
            WizardState::Saved {
                device: d,
                segments: 3,
            },
            WizardState::Failed {
                step: Step::Login,
                message: "x".into(),
            },
        ]
    }

    #[test]
    fn cancel_goes_idle_from_every_state() {
        for s in all_states() {
            assert_eq!(next(s, &Event::Cancel, 5), WizardState::Idle);
        }
    }

    #[test]
    fn retry_restarts_only_from_failed() {
        for s in all_states() {
            let want = if matches!(s, WizardState::Failed { .. }) {
                WizardState::Idle
            } else {
                s.clone()
            };
            assert_eq!(next(s, &Event::Retry, 5), want);
        }
    }

    #[test]
    fn choose_a_supported_device_starts_searching() {
        let s = WizardState::ChooseStrip {
            devices: vec![dev("a", true), dev("b", false)],
        };
        assert_eq!(
            next(s, &Event::Choose("a".into()), 42),
            WizardState::Searching {
                device: dev("a", true),
                since_us: 42,
                help: false
            }
        );
    }

    #[test]
    fn choose_unsupported_or_unknown_changes_nothing() {
        let s = WizardState::ChooseStrip {
            devices: vec![dev("a", true), dev("b", false)],
        };
        for id in ["b", "zzz", ""] {
            assert_eq!(next(s.clone(), &Event::Choose(id.into()), 1), s);
        }
    }

    #[test]
    fn choose_outside_choose_strip_changes_nothing() {
        for s in all_states() {
            if !matches!(s, WizardState::ChooseStrip { .. }) {
                assert_eq!(next(s.clone(), &Event::Choose("a".into()), 1), s);
            }
        }
    }

    #[test]
    fn state_serialises_with_kind_tag_and_camel_case() {
        let v = serde_json::to_value(WizardState::Searching {
            device: dev("a", true),
            since_us: 7,
            help: false,
        })
        .unwrap();
        assert_eq!(v["kind"], "searching");
        assert_eq!(v["sinceUs"], 7);
        assert_eq!(v["device"]["supported"], true);
    }
}
