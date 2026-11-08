//! LAN connection to the strip, its paint commands and the task that sends them (spec §5).

use std::fmt;
use std::net::IpAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use light_core::paint::{PAINT_DP, PaintCommand, encode};
use rustuya::tokio::{Device, Discovery};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands::StripStatus;
use crate::lan::is_allowed_lan_ip;

const TIMEOUT: Duration = Duration::from_secs(10);
/// Measured: the strip announces itself every ~5 s, but wifi drops some (gaps of 15 s seen).
const DISCOVERY_WINDOW: Duration = Duration::from_secs(30);

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
        log::warn!("strip file ignored: its mode must be 0600");
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

/// `<config dir>/strip.json`; debug builds let `DEEBOO_DEV_STRIP` point elsewhere.
#[must_use]
pub fn load(config_dir: &Path) -> Option<StripConfig> {
    #[cfg(debug_assertions)]
    if let Some(given) = std::env::var_os("DEEBOO_DEV_STRIP").map(std::path::PathBuf::from) {
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
        return cfg;
    }
    load_strip_file(&config_dir.join("strip.json"))
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
        let info = disco.find(&cfg.id, DISCOVERY_WINDOW).await.map_err(|e| {
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

/// What the strip task needs from a connected strip.
pub trait Painter {
    fn paint(
        &self,
        segments: u8,
        cmd: &PaintCommand,
    ) -> impl Future<Output = Result<(), StripError>> + Send;
}

impl Painter for Strip {
    fn paint(
        &self,
        segments: u8,
        cmd: &PaintCommand,
    ) -> impl Future<Output = Result<(), StripError>> + Send {
        Strip::paint(self, segments, cmd)
    }
}

/// From the engine to the strip task.
#[derive(Debug)]
pub enum StripMsg {
    Paint(PaintCommand),
    /// Try again after an error; ignored while connected.
    Connect,
}

/// Connects, then paints each command as it arrives. After a paint error it reports
/// `StripStatus::Error` and drops commands until a `Connect` brings the strip back.
pub async fn run<P, Fut>(
    name: String,
    segments: u8,
    mut connect: impl FnMut() -> Fut,
    mut rx: UnboundedReceiver<StripMsg>,
    report: impl Fn(StripStatus),
) where
    P: Painter,
    Fut: Future<Output = Result<P, StripError>>,
{
    let mut painter = None;
    loop {
        if painter.is_none() {
            report(StripStatus::Connecting);
            match connect().await {
                Ok(p) => {
                    // Whatever queued up while connecting is stale; the engine repaints on `Connected`.
                    while rx.try_recv().is_ok() {}
                    painter = Some(p);
                    report(StripStatus::Connected {
                        name: name.clone(),
                        segments,
                    });
                }
                Err(e) => {
                    log::error!("strip connection failed: {e}");
                    report(StripStatus::Error);
                }
            }
        }
        // Wait for the next message; while there is no painter only `Connect` matters.
        loop {
            let Some(msg) = rx.recv().await else { return };
            match (&painter, msg) {
                (Some(p), StripMsg::Paint(cmd)) => {
                    if let Err(e) = p.paint(segments, &cmd).await {
                        log::warn!("strip paint failed: {e}");
                        report(StripStatus::Error);
                        painter = None;
                    }
                }
                (None, StripMsg::Connect) => break,
                _ => {}
            }
        }
    }
}

/// Starts the strip task on the async runtime.
pub fn spawn(
    cfg: StripConfig,
    rx: UnboundedReceiver<StripMsg>,
    report: impl Fn(StripStatus) + Send + 'static,
) {
    tauri::async_runtime::spawn(async move {
        run(
            cfg.name.clone(),
            cfg.segments,
            || Strip::connect(&cfg),
            rx,
            report,
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

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

    /// Fails its second paint, whichever painter makes it.
    struct Fake(Arc<AtomicUsize>);

    impl Painter for Fake {
        fn paint(
            &self,
            _: u8,
            _: &PaintCommand,
        ) -> impl Future<Output = Result<(), StripError>> + Send {
            let failed = self.0.fetch_add(1, Ordering::SeqCst) == 1;
            std::future::ready(if failed {
                Err(StripError::Send)
            } else {
                Ok(())
            })
        }
    }

    async fn reports_reach(log: &Mutex<Vec<StripStatus>>, n: usize) {
        for _ in 0..2000 {
            if log.lock().map_or(0, |l| l.len()) >= n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    #[test]
    fn paint_error_marks_strip_and_stops_painting_until_reconnect() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let log = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (a, l, lg) = (Arc::clone(&attempts), Arc::clone(&log), Arc::clone(&log));
        let paint = || {
            StripMsg::Paint(PaintCommand::Bucket(light_core::paint::Hsv {
                h: 0,
                s: 0,
                v: 0,
            }))
        };
        tauri::async_runtime::block_on(async move {
            let task = tokio::spawn(run(
                "dj".into(),
                10,
                move || {
                    let a = Arc::clone(&a);
                    async move { Ok(Fake(a)) }
                },
                rx,
                move |s| l.lock().map(|mut v| v.push(s)).unwrap_or_default(),
            ));
            reports_reach(&lg, 2).await; // Connecting, Connected
            tx.send(paint()).unwrap_or_default(); // ok
            tx.send(paint()).unwrap_or_default(); // fails
            reports_reach(&lg, 3).await;
            tx.send(paint()).unwrap_or_default(); // dropped: no painter
            tx.send(StripMsg::Connect).unwrap_or_default();
            reports_reach(&lg, 5).await;
            tx.send(paint()).unwrap_or_default(); // painted again
            drop(tx);
            task.await.unwrap_or_default();
        });
        let connected = StripStatus::Connected {
            name: "dj".into(),
            segments: 10,
        };
        assert_eq!(
            *log.lock().unwrap(),
            vec![
                StripStatus::Connecting,
                connected.clone(),
                StripStatus::Error,
                StripStatus::Connecting,
                connected
            ]
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}
