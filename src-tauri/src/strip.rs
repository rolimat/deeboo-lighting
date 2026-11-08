//! LAN connection to the strip, its paint commands and the task that sends them (spec §5).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use light_core::paint::{Hsv, PAINT_DP, PaintCommand, decode_segment_count, encode};
use rustuya::tokio::{Device, Discovered, Discovery, Event, Listener, Version};
use secrecy::ExposeSecret;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands::StripStatus;
use crate::lan::on_local_subnet;
use crate::strip_store::{self, StripConfig};

const TIMEOUT: Duration = Duration::from_secs(10);
/// Measured: the strip announces itself every ~5 s, but wifi drops some (gaps of 15 s seen).
const DISCOVERY_WINDOW: Duration = Duration::from_secs(30);

/// Never carries the key, the address or file contents.
#[derive(Debug)]
pub enum StripError {
    NotFound,
    ForbiddenAddress,
    Connect,
    Send,
    Encode,
    #[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
    NoSegments,
}

impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFound => "strip not found on the LAN",
            Self::ForbiddenAddress => "strip address is not on a local private subnet",
            Self::Connect => "cannot connect to the strip",
            Self::Send => "cannot send to the strip",
            Self::Encode => "cannot encode the paint command",
            Self::NoSegments => "no paint data received from the strip",
        })
    }
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
        return load_logged(&path);
    }
    load_logged(&config_dir.join("strip.json"))
}

fn load_logged(path: &Path) -> Option<StripConfig> {
    strip_store::load(path)
        .inspect_err(|e| log::warn!("strip file ignored: {e}"))
        .ok()
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
        if !matches!(info.ip, IpAddr::V4(ip) if on_local_subnet(ip)) {
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

    /// Lights `segment` white on an otherwise dark strip so the user can see which it is.
    ///
    /// # Errors
    /// Fails if the segment is outside the strip or a message cannot be sent.
    #[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
    pub async fn identify(&self, segment: u8, segments: u8) -> Result<(), StripError> {
        identify_via(self, segment, segments).await
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

/// Gap between messages: the strip loses back-to-back ones (spec §5.3).
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
const MESSAGE_GAP: Duration = Duration::from_millis(125);
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
const MAX_SEGMENTS: u8 = 60;
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
const MAX_FRAME: usize = 4096;
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
const TUYA_PORT: u16 = 6668;

/// Where the strip answered; `version` is `Auto` when it was not announced.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
pub struct Found {
    pub ip: Ipv4Addr,
    pub version: Version,
}

#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
pub struct Announcement {
    pub id: String,
    pub ip: IpAddr,
    pub version: Option<Version>,
}

/// Source of discovery announcements (real: rustuya; tests: a script).
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
trait Announcements {
    fn next(&mut self) -> impl Future<Output = Option<Announcement>> + Send;
    fn reachable(&self, ip: Ipv4Addr) -> impl Future<Output = bool> + Send;
}

#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
async fn find_in(
    src: &mut impl Announcements,
    id: &str,
    last_ip: Option<Ipv4Addr>,
    window: Duration,
    allowed: impl Fn(Ipv4Addr) -> bool,
) -> Result<Found, StripError> {
    // The saved address is only a hint: it must still be allowed and answer.
    if let Some(ip) = last_ip
        && allowed(ip)
        && src.reachable(ip).await
    {
        return Ok(Found {
            ip,
            version: Version::Auto,
        });
    }
    let wait = async {
        while let Some(a) = src.next().await {
            if let IpAddr::V4(ip) = a.ip
                && a.id == id
                && allowed(ip)
            {
                return Some(Found {
                    ip,
                    version: a.version.unwrap_or(Version::Auto),
                });
            }
        }
        None
    };
    tokio::time::timeout(window, wait)
        .await
        .ok()
        .flatten()
        .ok_or(StripError::NotFound)
}

/// The strip's segment count from a status frame (`{"dps":{"61":"<base64>"}}`), if it holds one.
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
fn segments_from_frame(payload: &[u8]) -> Option<u8> {
    if payload.len() > MAX_FRAME {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let dps = v.get("dps").or_else(|| v.get("data")?.get("dps"))?;
    let raw = STANDARD.decode(dps.get("61")?.as_str()?).ok()?;
    decode_segment_count(&raw)
        .ok()
        .filter(|n| *n <= MAX_SEGMENTS)
}

/// Source of status frames (real: the device listener; tests: a script).
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
trait Frames {
    fn next(&mut self) -> impl Future<Output = Option<Vec<u8>>> + Send;
}

#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
async fn listen_in(frames: &mut impl Frames, window: Duration) -> Result<u8, StripError> {
    let wait = async {
        while let Some(f) = frames.next().await {
            if let Some(n) = segments_from_frame(&f) {
                return Some(n);
            }
        }
        None
    };
    tokio::time::timeout(window, wait)
        .await
        .ok()
        .flatten()
        .ok_or(StripError::NoSegments)
}

/// Eraser on every segment, then white on `segment` (spec §6.7.5).
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
async fn identify_via<P: Painter>(p: &P, segment: u8, segments: u8) -> Result<(), StripError> {
    let all = PaintCommand::Eraser {
        segments: (1..=segments).collect(),
    };
    let one = PaintCommand::Pencil {
        color: Hsv {
            h: 0,
            s: 0,
            v: 1000,
        },
        segments: vec![segment],
    };
    // Validate both before sending anything.
    encode(segments, &one).map_err(|_| StripError::Encode)?;
    p.paint(segments, &all).await?;
    tokio::time::sleep(MESSAGE_GAP).await;
    p.paint(segments, &one).await
}

#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
struct Disco {
    disco: Discovery,
    stream: Discovered,
}

impl Announcements for Disco {
    async fn next(&mut self) -> Option<Announcement> {
        let i = self.stream.recv().await.ok()?;
        Some(Announcement {
            id: i.id,
            ip: i.ip,
            version: i.version,
        })
    }

    async fn reachable(&self, ip: Ipv4Addr) -> bool {
        let _ = &self.disco;
        tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect((ip, TUYA_PORT)),
        )
        .await
        .is_ok_and(|r| r.is_ok())
    }
}

#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
struct DeviceFrames(Listener);

impl Frames for DeviceFrames {
    async fn next(&mut self) -> Option<Vec<u8>> {
        loop {
            if let Event::Frame(m) = self.0.recv().await? {
                return Some(m.payload);
            }
        }
    }
}

/// Finds the strip with this ID on the Mac's own subnet.
///
/// # Errors
/// `NotFound` if nothing valid answers within `window`.
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
pub async fn find_by_id(
    id: &str,
    last_ip: Option<Ipv4Addr>,
    window: Duration,
) -> Result<Found, StripError> {
    let disco = Discovery::new().map_err(|e| {
        log::debug!("discovery init failed: {e}");
        StripError::NotFound
    })?;
    let stream = disco.discovered();
    disco.request_scan();
    let mut src = Disco { disco, stream };
    find_in(&mut src, id, last_ip, window, on_local_subnet).await
}

/// Connects and waits for the first valid DP 61 frame (the user paints in Smart Life).
///
/// # Errors
/// `ForbiddenAddress`, `Connect`, or `NoSegments` when nothing valid arrives within `window`.
#[allow(dead_code, reason = "used by the wizard in Plan 4 task 6")]
pub async fn listen_segment_count(
    cfg: &StripConfig,
    ip: Ipv4Addr,
    window: Duration,
) -> Result<u8, StripError> {
    if !on_local_subnet(ip) {
        return Err(StripError::ForbiddenAddress);
    }
    let key = cfg.local_key.expose_secret().as_bytes().to_vec();
    let device = Device::builder(cfg.id.clone(), key)
        .address(ip.to_string())
        .connect()
        .map_err(|_| StripError::Connect)?;
    let mut frames = DeviceFrames(device.listener());
    let r = match device.wait_connected(TIMEOUT).await {
        Ok(()) => listen_in(&mut frames, window).await,
        Err(_) => Err(StripError::Connect),
    };
    device.close().await;
    r
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

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

    // --- finding the strip and learning its segments (fakes, simulated time) ---

    fn paused<F: Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .map_or_else(|_| unreachable!("runtime builds"), |rt| rt.block_on(f))
    }

    const MAC: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 5);
    const STRIP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 50);

    fn allowed(ip: Ipv4Addr) -> bool {
        crate::lan::is_allowed_lan_ip(ip, MAC, 24)
    }

    fn ann(id: &str, ip: [u8; 4]) -> Announcement {
        Announcement {
            id: id.into(),
            ip: IpAddr::V4(Ipv4Addr::from(ip)),
            version: Some(Version::V3_3),
        }
    }

    struct FakeAnn {
        queue: Vec<Announcement>,
        reachable: bool,
        probed: Mutex<Vec<Ipv4Addr>>,
    }

    impl FakeAnn {
        fn new(queue: Vec<Announcement>, reachable: bool) -> Self {
            Self {
                queue,
                reachable,
                probed: Mutex::default(),
            }
        }
    }

    impl Announcements for FakeAnn {
        async fn next(&mut self) -> Option<Announcement> {
            if self.queue.is_empty() {
                std::future::pending::<()>().await;
            }
            Some(self.queue.remove(0))
        }

        fn reachable(&self, ip: Ipv4Addr) -> impl Future<Output = bool> + Send {
            self.probed
                .lock()
                .map(|mut p| p.push(ip))
                .unwrap_or_default();
            std::future::ready(self.reachable)
        }
    }

    const WINDOW: Duration = Duration::from_secs(30);

    #[test]
    fn discovery_ignores_other_ids_and_foreign_ips() {
        let mut src = FakeAnn::new(
            vec![
                ann("other", [192, 168, 1, 60]),
                ann("mine", [8, 8, 8, 8]),
                ann("mine", [192, 168, 2, 50]),
                ann("mine", [192, 168, 1, 50]),
            ],
            false,
        );
        let found = paused(find_in(&mut src, "mine", None, WINDOW, allowed));
        assert_eq!(
            found.ok(),
            Some(Found {
                ip: STRIP,
                version: Version::V3_3
            })
        );
    }

    #[test]
    fn discovery_with_only_wrong_answers_times_out() {
        let mut src = FakeAnn::new(
            vec![ann("other", [192, 168, 1, 60]), ann("mine", [8, 8, 8, 8])],
            false,
        );
        let r = paused(find_in(&mut src, "mine", None, WINDOW, allowed));
        assert!(matches!(r, Err(StripError::NotFound)));
    }

    #[test]
    fn last_ip_tried_first_and_rechecked() {
        // Reachable and allowed: used without waiting for discovery.
        let mut src = FakeAnn::new(vec![], true);
        let r = paused(find_in(&mut src, "mine", Some(STRIP), WINDOW, allowed));
        assert_eq!(r.ok().map(|f| f.ip), Some(STRIP));
        // Foreign saved address: never probed, discovery decides.
        let mut src = FakeAnn::new(vec![ann("mine", [192, 168, 1, 51])], true);
        let r = paused(find_in(
            &mut src,
            "mine",
            Some(Ipv4Addr::new(8, 8, 8, 8)),
            WINDOW,
            allowed,
        ));
        assert_eq!(r.ok().map(|f| f.ip), Some(Ipv4Addr::new(192, 168, 1, 51)));
        assert!(src.probed.lock().unwrap().is_empty());
        // Saved address that does not answer: falls back to discovery.
        let mut src = FakeAnn::new(vec![ann("mine", [192, 168, 1, 52])], false);
        let r = paused(find_in(&mut src, "mine", Some(STRIP), WINDOW, allowed));
        assert_eq!(r.ok().map(|f| f.ip), Some(Ipv4Addr::new(192, 168, 1, 52)));
    }

    fn frame(dp61: &[u8]) -> Vec<u8> {
        format!(r#"{{"dps":{{"61":"{}"}}}}"#, STANDARD.encode(dp61)).into_bytes()
    }

    struct FakeFrames(Vec<Vec<u8>>);

    impl Frames for FakeFrames {
        async fn next(&mut self) -> Option<Vec<u8>> {
            if self.0.is_empty() {
                std::future::pending::<()>().await;
            }
            Some(self.0.remove(0))
        }
    }

    #[test]
    fn segment_listen_ignores_bad_payloads_and_times_out() {
        let bad = vec![
            b"not json".to_vec(),
            br#"{"dps":{"20":true}}"#.to_vec(),
            frame(&[0x00, 0x01]),
            frame(&[0x01, 0x01, 0x00, 0x0a]),
            frame(&[0x00, 0x01, 0x00, 0x00]),
            frame(&[0x00, 0x01, 0x00, 61]),
            br#"{"dps":{"61":"!!!"}}"#.to_vec(),
        ];
        let mut only_bad = FakeFrames(bad.clone());
        let r = paused(listen_in(&mut only_bad, Duration::from_secs(90)));
        assert!(matches!(r, Err(StripError::NoSegments)));
        let mut good = bad;
        good.push(frame(&[0x00, 0x02, 0x00, 0x0a, 0x01]));
        let mut mixed = FakeFrames(good);
        assert_eq!(
            paused(listen_in(&mut mixed, Duration::from_secs(90))).ok(),
            Some(10)
        );
    }

    #[test]
    fn frame_with_nested_dps_is_read() {
        let raw = format!(
            r#"{{"data":{{"dps":{{"61":"{}"}}}}}}"#,
            STANDARD.encode([0, 1, 0, 60])
        );
        assert_eq!(segments_from_frame(raw.as_bytes()), Some(60));
    }

    proptest::proptest! {
        #[test]
        fn arbitrary_frames_never_panic(raw in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..6000)) {
            if let Some(n) = segments_from_frame(&raw) {
                proptest::prop_assert!((1..=60).contains(&n));
            }
        }

        #[test]
        fn arbitrary_dp61_payloads_stay_in_range(dp in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..300)) {
            if let Some(n) = segments_from_frame(&frame(&dp)) {
                proptest::prop_assert!((1..=60).contains(&n));
            }
        }
    }

    struct Recorder(Mutex<Vec<(tokio::time::Instant, Vec<u8>)>>);

    impl Painter for Recorder {
        fn paint(
            &self,
            segments: u8,
            cmd: &PaintCommand,
        ) -> impl Future<Output = Result<(), StripError>> + Send {
            let bytes = encode(segments, cmd).map_err(|_| StripError::Encode);
            let r = bytes.map(|b| {
                self.0
                    .lock()
                    .map(|mut v| v.push((tokio::time::Instant::now(), b)))
                    .unwrap_or_default();
            });
            std::future::ready(r)
        }
    }

    fn hex(b: &[u8]) -> String {
        b.iter().fold(String::new(), |mut acc, x| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{x:02x}");
            acc
        })
    }

    #[test]
    fn identify_sends_eraser_then_pencil_with_gap() {
        let rec = Recorder(Mutex::default());
        assert!(paused(identify_via(&rec, 3, 10)).is_ok());
        let sent = rec.0.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(
            hex(&sent[0].1),
            "0001000a020000000000008a0102030405060708090a"
        );
        assert_eq!(hex(&sent[1].1), "0001000a010000000003e88103");
        assert!(sent[1].0 - sent[0].0 >= Duration::from_millis(125));
    }

    #[test]
    fn identify_rejects_a_segment_outside_the_strip() {
        let rec = Recorder(Mutex::default());
        assert!(matches!(
            paused(identify_via(&rec, 11, 10)),
            Err(StripError::Encode)
        ));
    }
}
