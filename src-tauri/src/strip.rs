//! LAN connection to the strip, its paint commands and the task that sends them (spec §5).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use light_core::paint::{Hsv, PAINT_DP, PaintCommand, decode_segment_count, encode};
use rustuya::tokio::{Device, Discovered, Discovery, Event, Listener, TuyaError, Version};
use secrecy::{ExposeSecret, SecretString};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands::StripStatus;
use crate::lan::on_local_subnet;
use crate::strip_store::{self, StoreError, StripConfig};

const TIMEOUT: Duration = Duration::from_secs(10);
/// How long a connected device has to answer the status query that proves it is our strip.
const VERIFY_WINDOW: Duration = Duration::from_secs(3);
/// Measured: the strip announces itself every ~5 s, but wifi drops some (gaps of 15 s seen).
const DISCOVERY_WINDOW: Duration = Duration::from_secs(30);

/// Automatic redials after a drop or a resume, and the wait before each.
const REDIAL_BACKOFF: [Duration; 5] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];

/// Never carries the key, the address or file contents.
#[derive(Debug)]
pub enum StripError {
    NotFound,
    ForbiddenAddress,
    Connect,
    /// The strip answered but rejected the key (decryption or authentication failed).
    Auth,
    Send,
    Encode,
    NoSegments,
}

impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFound => "strip not found on the LAN",
            Self::ForbiddenAddress => "strip address is not on a local private subnet",
            Self::Connect => "cannot connect to the strip",
            Self::Auth => "the strip rejected the saved key",
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
    load_logged(&store_path(config_dir))
}

/// Where the strip file lives (the dev override is for loading only).
#[must_use]
pub fn store_path(config_dir: &Path) -> std::path::PathBuf {
    config_dir.join("strip.json")
}

fn load_logged(path: &Path) -> Option<StripConfig> {
    strip_store::load(path)
        .inspect_err(|e| log::log!(load_level(e), "strip file ignored: {e}"))
        .ok()
}

/// A fresh install has no strip file; a corrupt or unreadable one must still be reported.
fn load_level(e: &StoreError) -> log::Level {
    if matches!(e, StoreError::Missing) {
        log::Level::Debug
    } else {
        log::Level::Warn
    }
}

pub struct Strip {
    device: Device,
    ip: Ipv4Addr,
}

/// What a connection attempt needs from the network (real: discovery and rustuya; tests: a script).
trait Link {
    type Conn;
    /// Whether the saved address is allowed on the Mac's subnets and answers.
    fn hint(&self, ip: Ipv4Addr) -> impl Future<Output = bool> + Send;
    /// Finds the strip by ID through discovery.
    fn find(&self, id: &str) -> impl Future<Output = Result<Found, StripError>> + Send;
    /// Asks the connected device for its status. Protocol 3.3 has no handshake, so a connection
    /// alone proves nothing about the device or the key.
    fn verify(&self, cfg: &StripConfig, conn: &Self::Conn) -> impl Future<Output = Verdict> + Send;
    /// Closes a connection: the strip takes one LAN client at a time.
    fn close(&self, conn: Self::Conn) -> impl Future<Output = ()> + Send;
    /// Persists the address the strip was found at (best effort).
    fn remember(&self, cfg: &StripConfig, ip: Ipv4Addr);
    /// `version` is `None` when unknown (the saved-address path).
    fn dial(
        &self,
        cfg: &StripConfig,
        ip: Ipv4Addr,
        version: Option<Version>,
    ) -> impl Future<Output = Result<Self::Conn, StripError>> + Send;
}

/// What a connected device made of the status query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// A decrypted reply from our device.
    Ours,
    /// A decrypted reply naming another device.
    Foreign,
    /// Our key failed to decrypt or authenticate what it sent: the only proof of a wrong key.
    Rejected,
    /// Nothing usable (some strips never answer a plain query): proves nothing either way.
    Silent,
}

/// The (device id, address) discovery confirmed for a strip that never answers a status query.
/// In memory, per session: a Configure with another id or address simply no longer matches it.
pub type SilentCache = Mutex<Option<(String, Ipv4Addr)>>;

fn silent_here(cache: &SilentCache, id: &str, ip: Ipv4Addr) -> bool {
    cache
        .lock()
        .is_ok_and(|c| c.as_ref().is_some_and(|(i, a)| i == id && *a == ip))
}

/// The saved address is only a hint: another Tuya device may hold it now. It is kept only when the
/// device answers the status query as ours; anything else falls back to discovery by ID. The device
/// found by ID is accepted unless it fails to authenticate (`Auth`) or answers as another device.
/// Returns the connection and the address it is at (to remember).
async fn connect_in<L: Link>(
    link: &L,
    cfg: &StripConfig,
    silent: &SilentCache,
) -> Result<(L::Conn, Ipv4Addr), StripError> {
    if let Some(ip) = cfg.last_ip
        && link.hint(ip).await
        && let Ok(conn) = link.dial(cfg, ip, None).await
    {
        let v = link.verify(cfg, &conn).await;
        // A strip that never answers was already tied to this address by discovery this session.
        if v == Verdict::Ours || (v == Verdict::Silent && silent_here(silent, &cfg.id, ip)) {
            return Ok((conn, ip));
        }
        link.close(conn).await;
    }
    let found = link.find(&cfg.id).await?;
    let version = (found.version != Version::Auto).then_some(found.version);
    let conn = link.dial(cfg, found.ip, version).await?;
    // Discovery already tied our ID to this address, so silence is not a reason to refuse it.
    match link.verify(cfg, &conn).await {
        Verdict::Ours => {}
        Verdict::Silent => {
            if let Ok(mut c) = silent.lock() {
                *c = Some((cfg.id.clone(), found.ip));
            }
        }
        v => {
            link.close(conn).await;
            return Err(if v == Verdict::Rejected {
                StripError::Auth
            } else {
                StripError::NotFound
            });
        }
    }
    if cfg.last_ip != Some(found.ip) {
        link.remember(cfg, found.ip);
    }
    Ok((conn, found.ip))
}

/// `Some(true)` for a decrypted status reply from our device (or one naming none), `Some(false)`
/// when it names another, `None` when it is not a status reply.
fn reply_kind(payload: &[u8], id: &str) -> Option<bool> {
    if payload.len() > MAX_FRAME {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    v.get("dps")
        .or_else(|| v.get("data")?.get("dps"))?
        .is_object()
        .then_some(())?;
    let named = |k: &str| v.get(k).and_then(|x| x.as_str()).is_none_or(|x| x == id);
    Some(named("devId") && named("gwId"))
}

/// `store` is the strip file to keep up to date when the strip turns up at a new address.
struct RealLink<'a> {
    store: Option<&'a Path>,
}

impl Link for RealLink<'_> {
    type Conn = Device;

    async fn hint(&self, ip: Ipv4Addr) -> bool {
        on_local_subnet(ip) && tcp_reachable(ip).await
    }

    async fn verify(&self, cfg: &StripConfig, device: &Device) -> Verdict {
        // Subscribe before asking so the reply cannot slip past.
        let mut frames = DeviceFrames(device.listener());
        let mut errors = device.watch_error();
        // A failure published before the subscription is ours: every dial builds a new `Device`.
        if errors.borrow_and_update().is_some() {
            return Verdict::Rejected;
        }
        if device.query().await.is_err() {
            return Verdict::Silent;
        }
        let wait = async {
            loop {
                tokio::select! {
                    f = frames.next() => {
                        // `None`: the device stopped; looping again would spin.
                        let Some(f) = f else { return Verdict::Silent };
                        if let Some(ours) = reply_kind(&f, &cfg.id) {
                            return if ours { Verdict::Ours } else { Verdict::Foreign };
                        }
                    }
                    e = errors.changed() => {
                        if e.is_err() {
                            return Verdict::Silent; // sender dropped
                        }
                        if errors.borrow().is_some() {
                            return Verdict::Rejected;
                        }
                    }
                }
            }
        };
        tokio::time::timeout(VERIFY_WINDOW, wait)
            .await
            .unwrap_or(Verdict::Silent)
    }

    async fn close(&self, device: Device) {
        device.close().await;
    }

    fn remember(&self, cfg: &StripConfig, ip: Ipv4Addr) {
        let Some(path) = self.store else { return };
        // Re-read so a newer segment count on disk is not overwritten by this task's copy.
        let saved = strip_store::load(path)
            .ok()
            .filter(|c| c.id == cfg.id)
            .map(|mut c| {
                c.last_ip = Some(ip);
                c
            })
            .map(|c| strip_store::save(path, &c));
        if !matches!(saved, Some(Ok(()))) {
            log::warn!("strip address not saved");
        }
    }

    async fn find(&self, id: &str) -> Result<Found, StripError> {
        find_by_id(id, None, DISCOVERY_WINDOW).await
    }

    async fn dial(
        &self,
        cfg: &StripConfig,
        ip: Ipv4Addr,
        version: Option<Version>,
    ) -> Result<Device, StripError> {
        let key = cfg.local_key.expose_secret().as_bytes().to_vec();
        // Only our state machine redials (re-checking the subnet each time); rustuya's own redial races it.
        let mut builder = Device::builder(cfg.id.clone(), key)
            .address(ip.to_string())
            .auto_reconnect(false);
        if let Some(v) = version {
            builder = builder.version(v);
        }
        let device = builder.connect().map_err(|_| StripError::Connect)?;
        if let Err(e) = device.wait_connected(TIMEOUT).await {
            device.close().await;
            return Err(match e {
                TuyaError::Core(c) if c.is_auth_failure() => StripError::Auth,
                _ => StripError::Connect,
            });
        }
        Ok(device)
    }
}

impl Strip {
    /// # Errors
    /// `NotFound` if discovery misses the ID, `Auth` if the strip rejects the key (or cannot be read with it), `Connect` otherwise.
    pub async fn connect(
        cfg: &StripConfig,
        store: Option<&Path>,
        silent: &SilentCache,
    ) -> Result<Self, StripError> {
        connect_in(&RealLink { store }, cfg, silent)
            .await
            .map(|(device, ip)| Self { device, ip })
    }

    /// Lights `segment` white on an otherwise dark strip so the user can see which it is.
    ///
    /// # Errors
    /// Fails if the segment is outside the strip or a message cannot be sent.
    pub async fn identify(&self, segment: u8, segments: u8) -> Result<(), StripError> {
        identify_via(self, segment, segments).await
    }

    /// # Errors
    /// Fails if the command does not encode or the message cannot be queued.
    pub async fn paint(&self, segments: u8, cmd: &PaintCommand) -> Result<(), StripError> {
        let bytes = encode(segments, cmd).map_err(|_| StripError::Encode)?;
        // A closed device fails at once instead of after rustuya's send timeout.
        if !self.device.is_connected() {
            return Err(StripError::Send);
        }
        self.device
            .set_value(PAINT_DP, STANDARD.encode(bytes))
            .await
            .map_err(|_| StripError::Send)
    }
}

/// Gap between messages: the strip loses back-to-back ones (spec §5.3).
pub(crate) const MESSAGE_GAP: Duration = Duration::from_millis(125);
const MAX_SEGMENTS: u8 = 60;
const MAX_FRAME: usize = 4096;
const TUYA_PORT: u16 = 6668;

/// Where the strip answered; `version` is `Auto` when it was not announced.
#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    pub ip: Ipv4Addr,
    pub version: Version,
}

pub struct Announcement {
    pub id: String,
    pub ip: IpAddr,
    pub version: Option<Version>,
}

/// Source of discovery announcements (real: rustuya; tests: a script).
trait Announcements {
    fn next(&mut self) -> impl Future<Output = Option<Announcement>> + Send;
    fn reachable(&self, ip: Ipv4Addr) -> impl Future<Output = bool> + Send;
}

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
trait Frames {
    fn next(&mut self) -> impl Future<Output = Option<Vec<u8>>> + Send;
}

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

/// Full-strip fills would light the unwired tail, so they become a pencil on the wired segments.
fn wired_cmd(c: &StripConfig, cmd: &PaintCommand) -> PaintCommand {
    match cmd {
        PaintCommand::Bucket(color) if c.segments < c.reported => PaintCommand::Pencil {
            color: *color,
            segments: (1..=c.segments).collect(),
        },
        other => other.clone(),
    }
}

pub(crate) fn connected_status(c: &StripConfig) -> StripStatus {
    StripStatus::Connected {
        name: c.name.clone(),
        segments: c.segments,
        reported: c.reported,
        ip: c.last_ip.filter(std::net::Ipv4Addr::is_private),
    }
}

/// Switches the unwired tail (N+1..=M) off; `None` when every reported segment is wired.
fn park_tail(c: &StripConfig) -> Option<PaintCommand> {
    (c.segments < c.reported).then(|| PaintCommand::Eraser {
        segments: (c.segments + 1..=c.reported).collect(),
    })
}

async fn park_unwired<P: Painter>(p: &P, c: &StripConfig) {
    if let Some(off) = park_tail(c) {
        if let Err(e) = p.paint(c.reported, &off).await {
            log::warn!("strip tail not switched off: {e}");
        }
        tokio::time::sleep(MESSAGE_GAP).await;
    }
}

/// Eraser on every segment, then white on `segment` (spec §6.7.5).
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
        tcp_reachable(ip).await
    }
}

async fn tcp_reachable(ip: Ipv4Addr) -> bool {
    tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::TcpStream::connect((ip, TUYA_PORT)),
    )
    .await
    .is_ok_and(|r| r.is_ok())
}

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
        .auto_reconnect(false)
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

    /// Where the strip was reached, if it can say.
    fn address(&self) -> Option<Ipv4Addr> {
        None
    }
}

impl Painter for Strip {
    fn paint(
        &self,
        segments: u8,
        cmd: &PaintCommand,
    ) -> impl Future<Output = Result<(), StripError>> + Send {
        Strip::paint(self, segments, cmd)
    }

    fn address(&self) -> Option<Ipv4Addr> {
        Some(self.ip)
    }
}

/// From the engine to the strip task.
pub enum StripMsg {
    Paint(PaintCommand),
    /// Try again after an error; ignored while connected.
    Connect,
    /// A new strip file: drop the connection and connect with this config.
    Configure(StripConfig),
    /// `true`: drop the connection and stay away (the wizard needs the strip, which accepts one
    /// LAN client); `false`: connect again.
    Pause(bool),
    /// Flash one segment white (1-based, already checked against the segment count).
    Identify(u8),
}

/// The status a failed connection shows: a rejected key and a missing ID get their own.
fn status_of(e: &StripError) -> StripStatus {
    match e {
        StripError::Auth => StripStatus::KeyInvalid,
        StripError::NotFound => StripStatus::NotFound,
        _ => StripStatus::Error,
    }
}

/// What matters while there is no connection; paints and retries meanwhile are stale and dropped.
enum Control {
    Configure(StripConfig),
    Pause(bool),
}

async fn next_control(rx: &mut UnboundedReceiver<StripMsg>) -> Option<Control> {
    loop {
        match rx.recv().await? {
            StripMsg::Configure(c) => return Some(Control::Configure(c)),
            StripMsg::Pause(p) => return Some(Control::Pause(p)),
            StripMsg::Paint(_) | StripMsg::Connect | StripMsg::Identify(_) => {}
        }
    }
}

/// Connects, then paints each command as it arrives. A dropped connection (paint/identify failure) or a
/// resume redials by itself with the saved config, up to 5 times with backoff, then repaints (the engine
/// replans on `Connected`). A failed first connection reports its status
/// (`StripStatus::{Error, NotFound, KeyInvalid}`) and drops commands until a `Connect` retries;
/// a `Configure` at any moment, even mid-connection, starts over with the new config.
/// Refills the redial budget after a dropped connection, less one redial per connection that
/// never painted; false once none is left.
fn redial_after_drop(retries: &mut usize, unproven: usize) -> bool {
    *retries = REDIAL_BACKOFF
        .len()
        .saturating_sub(unproven.saturating_sub(1));
    *retries > 0
}

#[allow(clippy::too_many_lines)] // one reconnect state machine; splitting it hides the flow
pub async fn run<P, Fut>(
    initial: Option<StripConfig>,
    mut connect: impl FnMut(Arc<StripConfig>) -> Fut,
    mut rx: UnboundedReceiver<StripMsg>,
    report: impl Fn(StripStatus),
) where
    P: Painter,
    Fut: Future<Output = Result<P, StripError>>,
{
    let mut cfg = initial.map(Arc::new);
    let mut painter = None;
    let mut paused = false;
    // An identify flash on an unwired segment leaves it lit; the next paint switches it off.
    let mut tail_lit = false;
    // Automatic redials left; 0 for a first connection, a Configure or a manual Connect.
    let mut retries = 0;
    // Connections made since the last successful paint; a strip that connects and then drops
    // at once must not refill the redial budget.
    let mut unproven = 0_usize;
    'connect: loop {
        // Paused: no connection and no reports; only a new config (kept) or the resume matter.
        while paused {
            match next_control(&mut rx).await {
                Some(Control::Configure(new)) => cfg = Some(Arc::new(new)),
                Some(Control::Pause(p)) => {
                    paused = p;
                    retries = if p { 0 } else { REDIAL_BACKOFF.len() };
                    unproven = 0;
                }
                None => return,
            }
        }
        let Some(c) = cfg.clone() else {
            report(StripStatus::NotConfigured);
            match next_control(&mut rx).await {
                Some(Control::Configure(new)) => cfg = Some(Arc::new(new)),
                Some(Control::Pause(p)) => paused = p,
                None => return,
            }
            continue;
        };
        if painter.is_none() {
            report(StripStatus::Connecting);
            if retries > 0 {
                let wait = REDIAL_BACKOFF[REDIAL_BACKOFF.len() - retries];
                retries -= 1;
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    new = next_control(&mut rx) => match new {
                        None => return,
                        Some(Control::Configure(n)) => {
                            cfg = Some(Arc::new(n));
                            retries = 0;
                            unproven = 0;
                            continue;
                        }
                        Some(Control::Pause(p)) => {
                            paused = p;
                            retries = if p { 0 } else { REDIAL_BACKOFF.len() };
                            unproven = 0;
                            continue;
                        }
                    },
                }
            }
            let attempt = tokio::select! {
                r = connect(c.clone()) => Ok(r),
                new = next_control(&mut rx) => Err(new),
            };
            match attempt {
                Err(None) => return,
                Err(Some(Control::Configure(new))) => {
                    cfg = Some(Arc::new(new));
                    retries = 0;
                    unproven = 0;
                    continue;
                }
                Err(Some(Control::Pause(p))) => {
                    paused = p;
                    retries = if p { 0 } else { REDIAL_BACKOFF.len() };
                    unproven = 0;
                    continue;
                }
                Ok(Ok(p)) => {
                    // Whatever queued up while connecting is stale (the engine repaints on
                    // `Connected`), except a newer config.
                    let mut newer = None;
                    while let Ok(m) = rx.try_recv() {
                        match m {
                            StripMsg::Configure(new) => newer = Some(new),
                            StripMsg::Pause(p) => paused = p,
                            StripMsg::Paint(_) | StripMsg::Connect | StripMsg::Identify(_) => {}
                        }
                    }
                    let restart = newer.is_some() || paused;
                    if let Some(new) = newer {
                        cfg = Some(Arc::new(new));
                        retries = 0;
                        unproven = 0;
                    }
                    if restart {
                        continue;
                    }
                    // Found at a new address: keep it for the redials of this session.
                    let c = match p.address() {
                        Some(ip) if c.last_ip != Some(ip) => {
                            let moved = Arc::new(StripConfig {
                                id: c.id.clone(),
                                name: c.name.clone(),
                                local_key: SecretString::from(
                                    c.local_key.expose_secret().to_owned(),
                                ),
                                last_ip: Some(ip),
                                ..*c
                            });
                            cfg = Some(Arc::clone(&moved));
                            moved
                        }
                        _ => Arc::clone(&c),
                    };
                    park_unwired(&p, &c).await;
                    tail_lit = false;
                    painter = Some(p);
                    retries = 0;
                    unproven += 1;
                    report(connected_status(&c));
                }
                Ok(Err(e)) => {
                    log::error!("strip connection failed: {e}");
                    if retries > 0 {
                        continue; // the next pass waits out its backoff, still `Connecting` for the UI
                    }
                    report(status_of(&e));
                }
            }
        }
        // Wait for the next message; while there is no painter only `Connect` matters.
        loop {
            let Some(msg) = rx.recv().await else { return };
            match (&painter, msg) {
                (_, StripMsg::Configure(new)) => {
                    cfg = Some(Arc::new(new));
                    painter = None;
                    continue 'connect;
                }
                (Some(_), StripMsg::Pause(false)) => {}
                // Dropping the painter closes the connection.
                (_, StripMsg::Pause(p)) => {
                    paused = p;
                    retries = if p { 0 } else { REDIAL_BACKOFF.len() };
                    unproven = 0;
                    painter = None;
                    continue 'connect;
                }
                (Some(p), StripMsg::Paint(cmd)) => {
                    if std::mem::take(&mut tail_lit) {
                        park_unwired(p, &c).await;
                    }
                    if let Err(e) = p.paint(c.reported, &wired_cmd(&c, &cmd)).await {
                        log::warn!("strip paint failed: {e}");
                        painter = None;
                        if !redial_after_drop(&mut retries, unproven) {
                            report(status_of(&e));
                            continue;
                        }
                        continue 'connect; // `Connecting` is reported, not a red blip
                    }
                    unproven = 0;
                }
                (Some(p), StripMsg::Identify(n)) => {
                    if let Err(e) = identify_via(p, n, c.reported).await {
                        log::warn!("strip identify failed: {e}");
                        painter = None;
                        if !redial_after_drop(&mut retries, unproven) {
                            report(status_of(&e));
                            continue;
                        }
                        continue 'connect; // `Connecting` is reported, not a red blip
                    }
                    tail_lit = n > c.segments;
                }
                (None, StripMsg::Connect) => {
                    unproven = 0;
                    continue 'connect;
                }
                _ => {}
            }
        }
    }
}

/// Starts the strip task on the async runtime; it idles until a config arrives if there is none.
pub fn spawn(
    cfg: Option<StripConfig>,
    store: Option<std::path::PathBuf>,
    rx: UnboundedReceiver<StripMsg>,
    report: impl Fn(StripStatus) + Send + 'static,
) {
    let silent = Arc::new(SilentCache::default());
    tauri::async_runtime::spawn(async move {
        run(
            cfg,
            move |c: Arc<StripConfig>| {
                let store = store.clone();
                let silent = Arc::clone(&silent);
                async move { Strip::connect(&c, store.as_deref(), &silent).await }
            },
            rx,
            report,
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn missing_strip_file_is_not_a_warning() {
        assert_eq!(load_level(&StoreError::Missing), log::Level::Debug);
        for e in [StoreError::Io, StoreError::Mode, StoreError::Invalid("id")] {
            assert_eq!(load_level(&e), log::Level::Warn);
        }
    }
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

    type Dial = std::pin::Pin<Box<dyn Future<Output = Result<Fake, StripError>> + Send>>;
    type Dials = Arc<Mutex<Vec<(String, Arc<AtomicUsize>)>>>;

    /// A connector that counts dials, remembers the key each used and plays `outcomes` in order
    /// (then connects); every connection is a `Fake` whose counter is kept.
    fn scripted(
        outcomes: Vec<Option<StripError>>,
    ) -> (Dials, impl FnMut(Arc<StripConfig>) -> Dial) {
        let dials: Dials = Arc::default();
        let d = Arc::clone(&dials);
        let mut outcomes = outcomes.into_iter();
        (dials, move |c| {
            let n = Arc::new(AtomicUsize::new(0));
            d.lock()
                .unwrap()
                .push((c.local_key.expose_secret().to_string(), Arc::clone(&n)));
            let r = match outcomes.next().flatten() {
                Some(e) => Err(e),
                None => Ok(Fake(n)),
            };
            Box::pin(async move { r })
        })
    }

    fn a_paint() -> StripMsg {
        StripMsg::Paint(PaintCommand::Bucket(light_core::paint::Hsv {
            h: 0,
            s: 0,
            v: 0,
        }))
    }

    async fn a_while() {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    fn connected10() -> StripStatus {
        StripStatus::Connected {
            name: "dj".into(),
            segments: 10,
            reported: 10,
            ip: None,
        }
    }

    #[test]
    fn lights_off_then_on_reconnects_with_the_saved_key() {
        // The off scene's paint finds the socket gone; the strip first refuses the redial (its old
        // session lingers), then accepts: no KeyInvalid, no wizard, the saved key each time.
        let (dials, connect) = scripted(vec![None, Some(StripError::Auth), None]);
        let r = drive_task(Some(cfg("dj", None)), connect, async |tx, _| {
            settle().await;
            tx.send(a_paint()).unwrap_or_default(); // ok
            tx.send(a_paint()).unwrap_or_default(); // fails: the lights-off paint
            a_while().await;
            tx.send(StripMsg::Connect).unwrap_or_default(); // lights on while connected: ignored
            tx.send(a_paint()).unwrap_or_default(); // the replan
            a_while().await;
        });
        assert!(!r.contains(&StripStatus::KeyInvalid), "{r:?}");
        assert_eq!(r.last(), Some(&connected10()));
        let d = dials.lock().unwrap();
        assert_eq!(d.len(), 3);
        assert!(d.iter().all(|(k, _)| k == "0123456789abcdef"));
        assert_eq!(d[2].1.load(Ordering::SeqCst), 1, "painted again");
    }

    #[test]
    fn a_failed_paint_reconnects_and_repaints() {
        let (dials, connect) = scripted(vec![]);
        let r = drive_task(Some(cfg("dj", None)), connect, async |tx, _| {
            settle().await;
            tx.send(a_paint()).unwrap_or_default();
            tx.send(a_paint()).unwrap_or_default(); // fails
            a_while().await;
            tx.send(a_paint()).unwrap_or_default();
            settle().await;
        });
        assert_eq!(
            r,
            vec![
                StripStatus::Connecting,
                connected10(),
                StripStatus::Connecting,
                connected10()
            ]
        );
        assert_eq!(dials.lock().unwrap()[1].1.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_closed_socket_is_not_a_rejected_key() {
        for e in [StripError::Send, StripError::Connect] {
            assert_eq!(status_of(&e), StripStatus::Error);
        }
        let (_, connect) = scripted(vec![None, Some(StripError::Connect)]);
        let r = drive_task(Some(cfg("dj", None)), connect, async |tx, _| {
            settle().await;
            tx.send(a_paint()).unwrap_or_default();
            tx.send(a_paint()).unwrap_or_default();
            a_while().await;
        });
        assert!(!r.contains(&StripStatus::KeyInvalid), "{r:?}");
    }

    #[test]
    fn a_connection_that_drops_at_once_does_not_reset_the_retries() {
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = Arc::clone(&n);
        let r = drive_task(
            Some(cfg("dj", None)),
            move |_| {
                n2.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(Fake(Arc::new(AtomicUsize::new(1)))) }) // the first paint fails
            },
            async |tx, _| {
                settle().await;
                for _ in 0..8 {
                    tx.send(a_paint()).unwrap_or_default();
                    a_while().await;
                }
            },
        );
        assert_eq!(n.load(Ordering::SeqCst), 6, "1 + 5 dials, then stop");
        assert_eq!(r.last(), Some(&StripStatus::Error));
    }

    #[test]
    fn retries_stop_after_five_attempts() {
        let mut outcomes = vec![None];
        outcomes.extend((0..9).map(|_| Some(StripError::Connect)));
        let (dials, connect) = scripted(outcomes);
        let r = drive_task(Some(cfg("dj", None)), connect, async |tx, _| {
            settle().await;
            tx.send(a_paint()).unwrap_or_default();
            tx.send(a_paint()).unwrap_or_default();
            a_while().await;
            assert_eq!(dials.lock().unwrap().len(), 6, "1 + 5 redials, then stop");
            tx.send(StripMsg::Connect).unwrap_or_default(); // Reconnect always tries again
            a_while().await;
            assert_eq!(dials.lock().unwrap().len(), 7);
        });
        assert_eq!(r.last(), Some(&StripStatus::Error));
    }

    #[test]
    fn a_new_config_mid_redial_never_retries_a_wrong_key() {
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = Arc::clone(&n);
        let r = drive_task(
            Some(cfg("dj", None)),
            move |_| match n2.fetch_add(1, Ordering::SeqCst) {
                0 => Box::pin(async { Ok(Fake(Arc::new(AtomicUsize::new(0)))) }),
                1 => Box::pin(std::future::pending()),
                _ => Box::pin(async { Err(StripError::Auth) }),
            },
            async |tx, _| {
                settle().await;
                tx.send(a_paint()).unwrap_or_default();
                tx.send(a_paint()).unwrap_or_default(); // fails: redial 1 hangs
                tokio::time::sleep(Duration::from_secs(1)).await;
                tx.send(StripMsg::Configure(cfg("new", None)))
                    .unwrap_or_default();
                a_while().await;
            },
        );
        assert_eq!(n.load(Ordering::SeqCst), 3, "the new config dials once");
        assert_eq!(r.last(), Some(&StripStatus::KeyInvalid));
    }

    #[test]
    fn configure_or_pause_cancels_a_backoff_wait() {
        for pause in [false, true] {
            let (dials, connect) = scripted(vec![
                None,
                Some(StripError::Connect),
                Some(StripError::Connect),
            ]);
            let _ = drive_task(Some(cfg("dj", None)), connect, async |tx, _| {
                settle().await;
                tx.send(a_paint()).unwrap_or_default();
                tx.send(a_paint()).unwrap_or_default();
                // Dial 2 fails at 0.5 s; the next wait is 1 s.
                tokio::time::sleep(Duration::from_millis(700)).await;
                assert_eq!(dials.lock().unwrap().len(), 2);
                let m = if pause {
                    StripMsg::Pause(true)
                } else {
                    StripMsg::Configure(cfg("new", None))
                };
                tx.send(m).unwrap_or_default();
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert_eq!(dials.lock().unwrap().len(), if pause { 2 } else { 3 });
            });
        }
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

    fn wired(n: u8, m: u8) -> StripConfig {
        StripConfig {
            segments: n,
            reported: m,
            ..cfg("w", None)
        }
    }

    #[test]
    fn header_keeps_reported_count() {
        let c = wired(8, 10);
        let red = Hsv {
            h: 0,
            s: 1000,
            v: 1000,
        };
        let bytes = encode(c.reported, &wired_cmd(&c, &PaintCommand::Bucket(red))).unwrap();
        // Header 10, pencil on 1..=8 only.
        assert_eq!(hex(&bytes), "0001000a01000003e803e8880102030405060708");
        let tail = park_tail(&c).unwrap();
        assert_eq!(
            hex(&encode(c.reported, &tail).unwrap()),
            "0001000a0200000000000082090a"
        );
        // Every segment wired: nothing changes.
        let full = wired(10, 10);
        assert_eq!(
            wired_cmd(&full, &PaintCommand::Bucket(red)),
            PaintCommand::Bucket(red)
        );
        assert!(park_tail(&full).is_none());
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

    struct Shared(Arc<Mutex<Vec<String>>>);

    impl Painter for Shared {
        fn paint(
            &self,
            segments: u8,
            cmd: &PaintCommand,
        ) -> impl Future<Output = Result<(), StripError>> + Send {
            if let Ok(b) = encode(segments, cmd) {
                self.0.lock().unwrap().push(hex(&b));
            }
            std::future::ready(Ok(()))
        }
    }

    #[test]
    fn a_lit_tail_segment_is_switched_off_before_the_next_paint() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let s = Arc::clone(&sent);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let red = Hsv {
            h: 0,
            s: 1000,
            v: 1000,
        };
        paused(async move {
            let task = run(
                Some(wired(8, 10)),
                move |_| {
                    let p = Shared(Arc::clone(&s));
                    async move { Ok(p) }
                },
                rx,
                |_| {},
            );
            let feed = async {
                settle().await;
                tx.send(StripMsg::Identify(9)).unwrap_or_default();
                tx.send(StripMsg::Paint(PaintCommand::Bucket(red)))
                    .unwrap_or_default();
                settle().await;
                drop(tx);
            };
            tokio::join!(task, feed);
        });
        let sent = sent.lock().unwrap().clone();
        let eraser_tail = "0001000a0200000000000082090a";
        // connect parks the tail once; identify 9 lights it; the next paint parks it again first.
        assert_eq!(
            sent.iter().filter(|m| *m == eraser_tail).count(),
            2,
            "{sent:?}"
        );
        let last = sent.len() - 1;
        assert_eq!(sent[last - 1], eraser_tail);
    }

    #[test]
    fn identify_rejects_a_segment_outside_the_strip() {
        let rec = Recorder(Mutex::default());
        assert!(matches!(
            paused(identify_via(&rec, 11, 10)),
            Err(StripError::Encode)
        ));
        assert!(
            rec.0.lock().unwrap().is_empty(),
            "nothing reached the strip"
        );
    }

    // --- reconnecting, classification and the stale-address fallback ---

    fn cfg(name: &str, last_ip: Option<Ipv4Addr>) -> StripConfig {
        StripConfig {
            id: "bf1234".into(),
            name: name.into(),
            local_key: secrecy::SecretString::from("0123456789abcdef"),
            segments: 10,
            reported: 10,
            last_ip,
        }
    }

    type Reports = Arc<Mutex<Vec<StripStatus>>>;

    /// Runs the task until `rx` closes; `script` feeds it messages and `connect` decides each outcome.
    fn drive_task(
        initial: Option<StripConfig>,
        connect: impl FnMut(
            Arc<StripConfig>,
        )
            -> std::pin::Pin<Box<dyn Future<Output = Result<Fake, StripError>> + Send>>,
        script: impl AsyncFnOnce(&tokio::sync::mpsc::UnboundedSender<StripMsg>, &Reports),
    ) -> Vec<StripStatus> {
        let log: Reports = Arc::default();
        let l = Arc::clone(&log);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        paused(async {
            let task = run(initial, connect, rx, move |s| {
                l.lock().map(|mut v| v.push(s)).unwrap_or_default();
            });
            let feed = async {
                script(&tx, &log).await;
                drop(tx);
            };
            tokio::join!(task, feed);
        });
        log.lock().map(|l| l.clone()).unwrap_or_default()
    }

    fn ok_fake(
        _: Arc<StripConfig>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Fake, StripError>> + Send>> {
        Box::pin(async { Ok(Fake(Arc::new(AtomicUsize::new(0)))) })
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[test]
    fn auth_failure_marks_key_invalid() {
        let r = drive_task(
            Some(cfg("dj", None)),
            |_| Box::pin(async { Err(StripError::Auth) }),
            async |_, _| settle().await,
        );
        assert_eq!(r, vec![StripStatus::Connecting, StripStatus::KeyInvalid]);
    }

    #[test]
    fn missing_id_marks_not_found() {
        let r = drive_task(
            Some(cfg("dj", None)),
            |_| Box::pin(async { Err(StripError::NotFound) }),
            async |tx, _| {
                settle().await;
                // Retrying asks again; a connection refused is a plain error.
                tx.send(StripMsg::Connect).unwrap_or_default();
                settle().await;
            },
        );
        assert_eq!(
            r,
            vec![
                StripStatus::Connecting,
                StripStatus::NotFound,
                StripStatus::Connecting,
                StripStatus::NotFound
            ]
        );
        assert_eq!(status_of(&StripError::Connect), StripStatus::Error);
    }

    #[test]
    fn configure_connects_a_strip_that_was_not_configured_and_replaces_a_connected_one() {
        let connected = |n: &str| StripStatus::Connected {
            name: n.into(),
            segments: 10,
            reported: 10,
            ip: None,
        };
        let r = drive_task(None, ok_fake, async |tx, _| {
            settle().await;
            tx.send(StripMsg::Configure(cfg("one", None)))
                .unwrap_or_default();
            settle().await;
            tx.send(StripMsg::Configure(cfg("two", None)))
                .unwrap_or_default();
            settle().await;
        });
        assert_eq!(
            r,
            vec![
                StripStatus::NotConfigured,
                StripStatus::Connecting,
                connected("one"),
                StripStatus::Connecting,
                connected("two")
            ]
        );
    }

    #[test]
    fn configure_wins_over_a_connection_attempt_in_flight() {
        let r = drive_task(
            Some(cfg("old", None)),
            |c| {
                if c.name == "old" {
                    Box::pin(std::future::pending())
                } else {
                    Box::pin(async { Ok(Fake(Arc::new(AtomicUsize::new(0)))) })
                }
            },
            async |tx, _| {
                settle().await;
                tx.send(StripMsg::Configure(cfg("new", None)))
                    .unwrap_or_default();
                settle().await;
            },
        );
        assert_eq!(
            r.last(),
            Some(&StripStatus::Connected {
                name: "new".into(),
                segments: 10,
                reported: 10,
                ip: None
            })
        );
    }

    #[test]
    fn pause_drops_the_connection_and_resume_connects_with_the_latest_config() {
        let dialled = Arc::new(Mutex::new(Vec::<String>::new()));
        let d = Arc::clone(&dialled);
        let connected = |n: &str| StripStatus::Connected {
            name: n.into(),
            segments: 10,
            reported: 10,
            ip: None,
        };
        let r = drive_task(
            Some(cfg("old", None)),
            move |c| {
                d.lock().unwrap().push(c.name.clone());
                Box::pin(async { Ok(Fake(Arc::new(AtomicUsize::new(0)))) })
            },
            async |tx, _| {
                settle().await;
                tx.send(StripMsg::Pause(true)).unwrap_or_default();
                settle().await;
                // While paused nothing dials: retries and paints are stale, a new config is kept.
                tx.send(StripMsg::Connect).unwrap_or_default();
                tx.send(StripMsg::Configure(cfg("new", None)))
                    .unwrap_or_default();
                settle().await;
                tx.send(StripMsg::Pause(false)).unwrap_or_default();
                // The resume waits out the first backoff (the strip may still hold the old session).
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
        );
        assert_eq!(*dialled.lock().unwrap(), ["old", "new"]);
        assert_eq!(
            r,
            vec![
                StripStatus::Connecting,
                connected("old"),
                StripStatus::Connecting,
                connected("new")
            ]
        );
    }

    #[test]
    fn pause_wins_over_a_connection_attempt_in_flight() {
        let dials = Arc::new(AtomicUsize::new(0));
        let d = Arc::clone(&dials);
        let r = drive_task(
            Some(cfg("dj", None)),
            move |_| {
                if d.fetch_add(1, Ordering::SeqCst) == 0 {
                    Box::pin(std::future::pending())
                } else {
                    Box::pin(async { Ok(Fake(Arc::new(AtomicUsize::new(0)))) })
                }
            },
            async |tx, _| {
                settle().await;
                tx.send(StripMsg::Pause(true)).unwrap_or_default();
                settle().await;
                assert_eq!(
                    dials.load(Ordering::SeqCst),
                    1,
                    "no second dial while paused"
                );
                tx.send(StripMsg::Pause(false)).unwrap_or_default();
                // The resume waits out the first backoff (the strip may still hold the old session).
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
        );
        assert!(matches!(r.last(), Some(StripStatus::Connected { .. })));
    }

    struct FakeLink {
        hint_ok: bool,
        /// Addresses whose device rejects the key.
        reject: Vec<Ipv4Addr>,
        found: Result<Ipv4Addr, ()>,
        dials: Mutex<Vec<Ipv4Addr>>,
        /// What each address's device does with the status query; unlisted ones answer as ours.
        verdicts: Vec<(Ipv4Addr, Verdict)>,
        finds: Mutex<usize>,
        saved: Mutex<Vec<Ipv4Addr>>,
        closed: Mutex<Vec<Ipv4Addr>>,
    }

    impl FakeLink {
        fn new(hint_ok: bool, reject: &[Ipv4Addr], found: Result<Ipv4Addr, ()>) -> Self {
            Self {
                hint_ok,
                reject: reject.to_vec(),
                found,
                dials: Mutex::default(),
                verdicts: Vec::new(),
                finds: Mutex::default(),
                saved: Mutex::default(),
                closed: Mutex::default(),
            }
        }

        fn says(mut self, ip: Ipv4Addr, v: Verdict) -> Self {
            self.verdicts.push((ip, v));
            self
        }
    }

    impl Link for FakeLink {
        type Conn = Ipv4Addr;

        fn hint(&self, _: Ipv4Addr) -> impl Future<Output = bool> + Send {
            std::future::ready(self.hint_ok)
        }

        fn find(&self, _: &str) -> impl Future<Output = Result<Found, StripError>> + Send {
            *self.finds.lock().unwrap() += 1;
            std::future::ready(
                self.found
                    .map(|ip| Found {
                        ip,
                        version: Version::Auto,
                    })
                    .map_err(|()| StripError::NotFound),
            )
        }

        fn verify(&self, _: &StripConfig, conn: &Ipv4Addr) -> impl Future<Output = Verdict> + Send {
            std::future::ready(
                self.verdicts
                    .iter()
                    .find(|(ip, _)| ip == conn)
                    .map_or(Verdict::Ours, |(_, v)| *v),
            )
        }

        fn close(&self, conn: Ipv4Addr) -> impl Future<Output = ()> + Send {
            self.closed
                .lock()
                .map(|mut d| d.push(conn))
                .unwrap_or_default();
            std::future::ready(())
        }

        fn remember(&self, _: &StripConfig, ip: Ipv4Addr) {
            self.saved
                .lock()
                .map(|mut d| d.push(ip))
                .unwrap_or_default();
        }

        fn dial(
            &self,
            _: &StripConfig,
            ip: Ipv4Addr,
            _: Option<Version>,
        ) -> impl Future<Output = Result<Ipv4Addr, StripError>> + Send {
            self.dials
                .lock()
                .map(|mut d| d.push(ip))
                .unwrap_or_default();
            std::future::ready(if self.reject.contains(&ip) {
                Err(StripError::Auth)
            } else {
                Ok(ip)
            })
        }
    }

    const STALE: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 77);

    struct Moved;

    impl Painter for Moved {
        fn paint(
            &self,
            _: u8,
            _: &PaintCommand,
        ) -> impl Future<Output = Result<(), StripError>> + Send {
            std::future::ready(Ok(()))
        }

        fn address(&self) -> Option<Ipv4Addr> {
            Some(STRIP)
        }
    }

    #[test]
    fn the_task_keeps_the_address_the_strip_was_found_at() {
        let seen: Arc<Mutex<Vec<Option<Ipv4Addr>>>> = Arc::default();
        let s = Arc::clone(&seen);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let reports: Reports = Arc::default();
        let r = Arc::clone(&reports);
        paused(async {
            let task = run(
                Some(cfg("dj", Some(STALE))),
                |c: Arc<StripConfig>| {
                    s.lock().unwrap().push(c.last_ip);
                    std::future::ready(Ok(Moved))
                },
                rx,
                move |st| r.lock().unwrap().push(st),
            );
            let feed = async {
                settle().await;
                // Lights back on: a fresh connect must start from the new address.
                tx.send(StripMsg::Pause(true)).unwrap();
                tx.send(StripMsg::Pause(false)).unwrap();
                tokio::time::sleep(Duration::from_secs(1)).await;
                drop(tx);
            };
            tokio::join!(task, feed);
        });
        assert_eq!(*seen.lock().unwrap(), [Some(STALE), Some(STRIP)]);
        let ip = |s: &StripStatus| match s {
            StripStatus::Connected { ip, .. } => Some(*ip),
            _ => None,
        };
        assert_eq!(
            ip(reports
                .lock()
                .unwrap()
                .iter()
                .find(|s| ip(s).is_some())
                .unwrap()),
            Some(Some(STRIP))
        );
    }

    fn go(link: &FakeLink, c: &StripConfig) -> Result<Ipv4Addr, StripError> {
        paused(connect_in(link, c, &SilentCache::default())).map(|(conn, _)| conn)
    }

    fn saved(l: &FakeLink) -> Vec<Ipv4Addr> {
        l.saved.lock().unwrap().clone()
    }

    #[test]
    fn another_key_or_device_at_the_saved_ip_goes_to_discovery_and_saves_the_new_ip() {
        for v in [Verdict::Rejected, Verdict::Foreign, Verdict::Silent] {
            let link = FakeLink::new(true, &[], Ok(STRIP)).says(STALE, v);
            assert_eq!(go(&link, &cfg("dj", Some(STALE))).ok(), Some(STRIP));
            assert_eq!(*link.dials.lock().unwrap(), [STALE, STRIP]);
            assert_eq!(saved(&link), [STRIP]);
            assert_eq!(*link.closed.lock().unwrap(), [STALE]);
        }
    }

    #[test]
    fn a_silent_strip_at_the_saved_ip_is_accepted_once_discovery_confirms_the_address() {
        let link = FakeLink::new(true, &[], Ok(STALE)).says(STALE, Verdict::Silent);
        assert_eq!(go(&link, &cfg("dj", Some(STALE))).ok(), Some(STALE));
        assert_eq!(saved(&link), Vec::<Ipv4Addr>::new());
        // The first connection was closed before the second dial.
        assert_eq!(*link.closed.lock().unwrap(), [STALE]);
    }

    #[test]
    fn a_confirmed_silent_strip_is_not_rediscovered_on_the_next_connect() {
        let link = FakeLink::new(true, &[], Ok(STALE)).says(STALE, Verdict::Silent);
        let cache = SilentCache::default();
        let c = cfg("dj", Some(STALE));
        paused(connect_in(&link, &c, &cache)).unwrap();
        assert_eq!(*link.finds.lock().unwrap(), 1);
        paused(connect_in(&link, &c, &cache)).unwrap();
        assert_eq!(*link.finds.lock().unwrap(), 1);
        // Another device id at the same address is not covered by the cache.
        let other = StripConfig {
            id: "other".into(),
            ..cfg("dj", Some(STALE))
        };
        paused(connect_in(&link, &other, &cache)).unwrap();
        assert_eq!(*link.finds.lock().unwrap(), 2);
    }

    #[test]
    fn a_silent_strip_found_at_a_new_ip_is_accepted_and_saved() {
        let link = FakeLink::new(false, &[], Ok(STRIP)).says(STRIP, Verdict::Silent);
        let (conn, ip) = paused(connect_in(
            &link,
            &cfg("dj", Some(STALE)),
            &SilentCache::default(),
        ))
        .unwrap();
        assert_eq!((conn, ip), (STRIP, STRIP));
        assert_eq!(saved(&link), [STRIP]);
    }

    #[test]
    fn only_a_decrypt_failure_on_the_device_found_by_id_is_a_rejected_key() {
        let link = FakeLink::new(true, &[], Ok(STRIP))
            .says(STALE, Verdict::Silent)
            .says(STRIP, Verdict::Rejected);
        assert!(matches!(
            go(&link, &cfg("dj", Some(STALE))),
            Err(StripError::Auth)
        ));
        assert_eq!(saved(&link), Vec::<Ipv4Addr>::new());
        assert_eq!(*link.closed.lock().unwrap(), [STALE, STRIP]);
        // Another device's reply at the found address is not our strip at all.
        let link = FakeLink::new(true, &[], Ok(STRIP)).says(STRIP, Verdict::Foreign);
        assert!(matches!(
            go(&link, &cfg("dj", None)),
            Err(StripError::NotFound)
        ));
    }

    #[test]
    fn a_healthy_saved_ip_neither_discovers_nor_rewrites() {
        let link = FakeLink::new(true, &[], Err(()));
        assert!(go(&link, &cfg("dj", Some(STALE))).is_ok());
        assert_eq!(saved(&link), Vec::<Ipv4Addr>::new());
        assert_eq!(*link.closed.lock().unwrap(), Vec::<Ipv4Addr>::new());
    }

    #[test]
    fn a_status_reply_is_ours_only_with_dps_and_our_id() {
        let kind = |p: &str| reply_kind(p.as_bytes(), "bf1234");
        assert_eq!(kind(r#"{"dps":{"20":true}}"#), Some(true));
        assert_eq!(kind(r#"{"devId":"bf1234","dps":{"20":true}}"#), Some(true));
        assert_eq!(kind(r#"{"data":{"dps":{"20":true}}}"#), Some(true));
        assert_eq!(kind(r#"{"devId":"other","dps":{"20":true}}"#), Some(false));
        assert_eq!(kind(r#"{"dps":[]}"#), None);
        assert_eq!(kind(r#"{"x":1}"#), None);
        assert_eq!(kind("garbage"), None);
    }

    #[test]
    fn remember_updates_only_last_ip_of_the_same_strip_and_keeps_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("strip.json");
        let mut c = cfg("dj", Some(STALE));
        c.segments = 7;
        strip_store::save(&path, &c).unwrap();
        let link = RealLink { store: Some(&path) };
        link.remember(&c, STRIP);
        let back = strip_store::load(&path).unwrap();
        assert_eq!(
            (
                back.segments,
                back.last_ip,
                back.local_key.expose_secret().to_owned()
            ),
            (7, Some(STRIP), "0123456789abcdef".to_owned())
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Another strip's file is left alone.
        let mut other = cfg("dj", Some(STALE));
        other.id = "bfffff".into();
        link.remember(&other, Ipv4Addr::new(192, 168, 1, 99));
        assert_eq!(strip_store::load(&path).unwrap().last_ip, Some(STRIP));
    }

    #[test]
    fn stale_last_ip_falls_back_to_discovery() {
        // Another Tuya device answers on the saved address and refuses our key.
        let link = FakeLink::new(true, &[STALE], Ok(STRIP));
        let r = go(&link, &cfg("dj", Some(STALE)));
        assert_eq!(r.ok(), Some(STRIP));
        assert_eq!(*link.dials.lock().unwrap(), [STALE, STRIP]);
    }

    #[test]
    fn only_the_device_found_by_id_can_fail_the_key() {
        let link = FakeLink::new(true, &[STALE, STRIP], Ok(STRIP));
        let r = go(&link, &cfg("dj", Some(STALE)));
        assert!(matches!(r, Err(StripError::Auth)));
        assert_eq!(*link.dials.lock().unwrap(), [STALE, STRIP]);
        // A rejected key on the saved address alone, with the strip not announcing: not found.
        let link = FakeLink::new(true, &[STALE], Err(()));
        let r = go(&link, &cfg("dj", Some(STALE)));
        assert!(matches!(r, Err(StripError::NotFound)));
    }

    #[test]
    fn unreachable_or_missing_hint_goes_straight_to_discovery() {
        let link = FakeLink::new(false, &[], Ok(STRIP));
        assert_eq!(go(&link, &cfg("dj", Some(STALE))).ok(), Some(STRIP));
        assert_eq!(*link.dials.lock().unwrap(), [STRIP]);
        let link = FakeLink::new(true, &[], Ok(STRIP));
        assert_eq!(go(&link, &cfg("dj", None)).ok(), Some(STRIP));
        assert_eq!(*link.dials.lock().unwrap(), [STRIP]);
    }

    #[test]
    fn a_good_saved_address_is_used_without_discovery() {
        let link = FakeLink::new(true, &[], Err(()));
        assert_eq!(go(&link, &cfg("dj", Some(STALE))).ok(), Some(STALE));
    }
}
