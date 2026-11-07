//! Debug-only LAN connection to the strip and its paint commands (spec §5).

use std::fmt;
use std::net::IpAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use light_core::paint::{Hsv, PAINT_DP, PaintCommand, encode};
use rustuya::tokio::{Device, Discovery};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::commands::{AppState, StripStatus};
use crate::lan::is_allowed_lan_ip;

const TIMEOUT: Duration = Duration::from_secs(10);
/// Measured: messages closer than this get lost or reordered (tuya-protocol skill).
const MESSAGE_GAP: Duration = Duration::from_millis(125);
const STEP: Duration = Duration::from_secs(1);

pub struct StripConfig {
    pub id: String,
    pub name: String,
    pub local_key: SecretString,
    pub segments: u8,
}

#[derive(Deserialize)]
struct StripFile {
    id: String,
    name: String,
    local_key: String,
    segments: Option<u8>,
}

/// Never carries the key, the address or file contents.
#[derive(Debug)]
pub enum StripError {
    NotFound,
    ForbiddenAddress,
    Connect,
    Send,
    Encode,
}

impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFound => "strip not found on the LAN",
            Self::ForbiddenAddress => "strip address is not on a local private subnet",
            Self::Connect => "cannot connect to the strip",
            Self::Send => "cannot send to the strip",
            Self::Encode => "cannot encode the paint command",
        })
    }
}

/// Refuses files that are not mode 0600: they hold the strip's local key.
fn load_strip_file(path: &Path) -> Option<StripConfig> {
    let mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
    if mode != 0o600 {
        log::warn!("dev strip file ignored: its mode must be 0600");
        return None;
    }
    let file: StripFile = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    Some(StripConfig {
        id: file.id,
        name: file.name,
        local_key: SecretString::from(file.local_key),
        segments: file.segments.unwrap_or(10),
    })
}

fn load_dev_strip() -> Option<StripConfig> {
    let given = std::path::PathBuf::from(std::env::var_os("DEEBOO_DEV_STRIP")?);
    // `tauri dev` runs the binary from src-tauri/, so a relative path means repo-root relative.
    let path = if given.is_relative() {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(given)
    } else {
        given
    };
    let cfg = load_strip_file(&path);
    if cfg.is_none() {
        log::warn!("DEEBOO_DEV_STRIP is set but the strip file is missing or invalid");
    }
    cfg
}

pub struct Strip {
    device: Device,
}

impl Strip {
    /// # Errors
    /// Fails if the strip is not found, sits outside the local subnets, or does not connect.
    pub async fn connect(cfg: &StripConfig) -> Result<Self, StripError> {
        let disco = Discovery::new().map_err(|e| {
            log::debug!("discovery init failed: {e}");
            StripError::NotFound
        })?;
        let info = disco.find(&cfg.id, TIMEOUT).await.map_err(|e| {
            log::debug!("discovery failed: {e}");
            StripError::NotFound
        })?;
        if !on_local_subnet(info.ip) {
            return Err(StripError::ForbiddenAddress);
        }
        let key = cfg.local_key.expose_secret().as_bytes().to_vec();
        let mut builder = Device::builder(cfg.id.clone(), key).address(info.ip.to_string());
        if let Some(v) = info.version {
            builder = builder.version(v);
        }
        let device = builder.connect().map_err(|_| StripError::Connect)?;
        device
            .wait_connected(TIMEOUT)
            .await
            .map_err(|_| StripError::Connect)?;
        Ok(Self { device })
    }

    /// # Errors
    /// Fails if the command does not encode or the message cannot be queued.
    pub async fn paint(&self, segments: u8, cmd: &PaintCommand) -> Result<(), StripError> {
        let bytes = encode(segments, cmd).map_err(|_| StripError::Encode)?;
        self.device
            .set_value(PAINT_DP, STANDARD.encode(bytes))
            .await
            .map_err(|_| StripError::Send)
    }

    /// Odd red, even white, segment 1 off, all blue; one message at a time.
    ///
    /// # Errors
    /// Fails on the first message that cannot be sent.
    pub async fn paint_test(&self, segments: u8) -> Result<(), StripError> {
        let red = Hsv {
            h: 0,
            s: 1000,
            v: 1000,
        };
        let white = Hsv {
            h: 0,
            s: 0,
            v: 1000,
        };
        let blue = Hsv {
            h: 240,
            s: 1000,
            v: 1000,
        };
        let steps = [
            PaintCommand::Pencil {
                color: red,
                segments: (1..=segments).step_by(2).collect(),
            },
            PaintCommand::Pencil {
                color: white,
                segments: (2..=segments).step_by(2).collect(),
            },
            PaintCommand::Eraser { segments: vec![1] },
            PaintCommand::Bucket(blue),
        ];
        for (i, cmd) in steps.iter().enumerate() {
            if i > 0 {
                tokio::time::sleep(STEP.max(MESSAGE_GAP)).await;
            }
            self.paint(segments, cmd).await?;
        }
        Ok(())
    }
}

fn on_local_subnet(ip: IpAddr) -> bool {
    let IpAddr::V4(ip) = ip else { return false };
    if_addrs::get_if_addrs().is_ok_and(|ifaces| {
        ifaces.iter().any(|i| match &i.addr {
            if_addrs::IfAddr::V4(a) => is_allowed_lan_ip(
                ip,
                a.ip,
                u8::try_from(u32::from(a.netmask).leading_ones()).unwrap_or(0),
            ),
            if_addrs::IfAddr::V6(_) => false,
        })
    })
}

/// Connects in the background when `DEEBOO_DEV_STRIP` points at a valid strip file.
pub fn start_dev_connection(app: &AppHandle) {
    let Some(cfg) = load_dev_strip() else { return };
    let state = app.state::<AppState>();
    state.set_strip_status(StripStatus::Connecting);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match Strip::connect(&cfg).await {
            Ok(strip) => {
                state.set_strip(Arc::new(strip));
                state.set_strip_status(StripStatus::Connected {
                    name: cfg.name.clone(),
                    segments: cfg.segments,
                });
                log::info!("strip connected");
            }
            Err(e) => {
                log::error!("strip connection failed: {e}");
                state.set_strip_status(StripStatus::Error);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, mode: u32) -> std::path::PathBuf {
        let p = dir.join("strip.json");
        fs::write(&p, r#"{"id":"abc","name":"dj lights","local_key":"k"}"#).unwrap_or_default();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap_or_default();
        p
    }

    #[test]
    fn loads_a_0600_file_with_default_segments() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = load_strip_file(&write(dir.path(), 0o600)).unwrap();
        assert_eq!(
            (cfg.id.as_str(), cfg.name.as_str(), cfg.segments),
            ("abc", "dj lights", 10)
        );
    }

    #[test]
    fn refuses_a_file_readable_by_others() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_strip_file(&write(dir.path(), 0o644)).is_none());
    }
}
