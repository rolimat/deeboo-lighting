//! Drives one wizard run: cloud login, then the LAN steps, then identify and save.
//! The pure state machine lives in `state`, the cloud half in `cloud`; this is the glue the
//! IPC commands call. Nothing here logs or emits a secret.

use std::future::Future;
use std::net::Ipv4Addr;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use secrecy::SecretString;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task::AbortHandle;
use tuya_cloud::{Transport, UserCode};

use crate::strip::{self, Strip, StripError};
use crate::strip_store::{self, StripConfig};
use crate::wizard::cloud::{self, Input};
use crate::wizard::state::{DeviceDto, Step, WizardState};

/// One discovery attempt; after the first miss the UI shows the Bluetooth / 2.4 GHz help.
pub const FIND_WINDOW: Duration = Duration::from_secs(30);
pub const LISTEN_WINDOW: Duration = Duration::from_secs(90);
/// Pause after a failed attempt so a broken socket cannot spin the loop.
const RETRY_PAUSE: Duration = Duration::from_secs(1);

const NO_PAINT: &str = "No paint data received. The strip may not be compatible.";
const SAVE_FAILED: &str = "Could not save the strip. Try again.";

pub type Emit = Arc<dyn Fn(WizardState) + Send + Sync>;
type Transp = Arc<dyn Transport + Send + Sync>;
type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A connected strip that can light one segment.
pub trait Identify: Send + Sync {
    fn identify(&self, segment: u8, segments: u8) -> BoxFut<'_, Result<(), StripError>>;
}

/// The LAN side of the wizard (real: `strip`; tests: a script).
pub trait Lan: Send + Sync + 'static {
    fn find(
        &self,
        id: &str,
        window: Duration,
    ) -> impl Future<Output = Result<Ipv4Addr, StripError>> + Send;
    fn listen(
        &self,
        cfg: &StripConfig,
        ip: Ipv4Addr,
        window: Duration,
    ) -> impl Future<Output = Result<u8, StripError>> + Send;
    fn connect(
        &self,
        cfg: &StripConfig,
    ) -> impl Future<Output = Result<Arc<dyn Identify>, StripError>> + Send;
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    #[default]
    Idle,
    Running,
    Identify,
}

struct Ready {
    cfg: StripConfig,
    device: DeviceDto,
    conn: Arc<dyn Identify>,
}

#[derive(Default)]
struct Inner {
    phase: Phase,
    inputs: Option<Sender<Input>>,
    lan: Option<AbortHandle>,
    ready: Option<Ready>,
    /// Set by a cancel during the cloud phase, so the hand-over to the LAN phase honours it.
    cancelled: bool,
}

/// The one wizard of the app. Cheap to clone; all clones share the run.
#[derive(Clone, Default)]
pub struct Wizard(Arc<Mutex<Inner>>);

fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_micros()).ok())
        .unwrap_or(0)
}

/// Device ids from the UI: 1..=64 ASCII alphanumerics.
fn valid_device_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The store refuses control characters and names over 63 chars; a cloud name must not break saving.
fn store_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(63).collect()
}

impl Wizard {
    /// Starts a run: cloud login, then the LAN steps. Progress goes out through `emit`.
    ///
    /// # Errors
    /// Fails if a run is already in progress.
    pub fn start<L: Lan>(
        &self,
        lan: Arc<L>,
        transport: Transp,
        code: UserCode,
        emit: Emit,
    ) -> Result<(), String> {
        let (tx, rx) = channel(8);
        {
            let mut g = lock(&self.0);
            if g.phase != Phase::Idle {
                return Err("The wizard is already running.".into());
            }
            *g = Inner {
                phase: Phase::Running,
                inputs: Some(tx),
                ..Inner::default()
            };
        }
        let me = self.clone();
        tokio::spawn(async move { me.drive(lan, transport, code, emit, rx).await });
        Ok(())
    }

    async fn drive<L: Lan>(
        self,
        lan: Arc<L>,
        transport: Transp,
        code: UserCode,
        emit: Emit,
        rx: Receiver<Input>,
    ) {
        let e = emit.clone();
        // `cloud::run` has already emitted `Failed` or `Idle` when it returns an error.
        let Ok((device, key)) = cloud::run(transport, code, move |s| e(s), rx).await else {
            self.finish();
            return;
        };
        let task = {
            let emit = emit.clone();
            tokio::spawn(async move { lan_phase(&*lan, device, key, &emit).await })
        };
        {
            let mut g = lock(&self.0);
            // A cancel that landed between the two phases found nothing to abort.
            if g.cancelled {
                task.abort();
            }
            g.lan = Some(task.abort_handle());
        }
        match task.await {
            Ok(Ok(ready)) => {
                let mut g = lock(&self.0);
                g.phase = Phase::Identify;
                g.inputs = None;
                g.lan = None;
                g.ready = Some(ready);
            }
            Ok(Err((step, message))) => {
                emit(WizardState::Failed { step, message });
                self.finish();
            }
            Err(_aborted) => {
                emit(WizardState::Idle);
                self.finish();
            }
        }
    }

    fn finish(&self) {
        *lock(&self.0) = Inner::default();
    }

    /// Picks the strip from the list; unsupported ids are refused by the flow.
    ///
    /// # Errors
    /// Fails on a malformed id or when no run is waiting for a choice.
    pub fn choose(&self, id: &str) -> Result<(), String> {
        if !valid_device_id(id) {
            return Err("Invalid device.".into());
        }
        let g = lock(&self.0);
        g.inputs
            .as_ref()
            .and_then(|tx| tx.try_send(Input::Choose(id.to_owned())).ok())
            .ok_or_else(|| "No choice is pending.".to_owned())
    }

    /// Cancels whatever is running; always ends in `Idle`.
    pub fn cancel(&self, emit: &Emit) {
        let mut g = lock(&self.0);
        if g.phase == Phase::Running {
            // The running flow revokes the cloud session and emits `Idle` itself.
            g.cancelled = true;
            if let Some(tx) = &g.inputs {
                let _ = tx.try_send(Input::Cancel);
            }
            if let Some(h) = g.lan.take() {
                h.abort();
            }
            return;
        }
        *g = Inner::default();
        drop(g);
        emit(WizardState::Idle);
    }

    /// Back to the first step after a failure.
    pub fn retry(&self, emit: &Emit) {
        if lock(&self.0).phase == Phase::Idle {
            emit(WizardState::Idle);
        }
    }

    /// Lights `segment` (1-based) white on the strip.
    ///
    /// # Errors
    /// Fails outside the identify step or for a segment outside the strip.
    pub async fn identify(&self, segment: u8) -> Result<(), String> {
        let (conn, segments) = {
            let g = lock(&self.0);
            let r = g
                .ready
                .as_ref()
                .filter(|_| g.phase == Phase::Identify)
                .ok_or("Not ready to identify.")?;
            (r.conn.clone(), r.cfg.segments)
        };
        if !(1..=segments).contains(&segment) {
            return Err("No such segment.".into());
        }
        conn.identify(segment, segments).await.map_err(|e| {
            log::debug!("identify failed: {e}");
            "Could not light that segment.".to_owned()
        })
    }

    /// Writes the strip file (0600) and finishes.
    ///
    /// # Errors
    /// Fails outside the identify step or when the file cannot be written.
    pub fn save(&self, path: &Path, emit: &Emit) -> Result<(), String> {
        let ready = {
            let mut g = lock(&self.0);
            if g.phase != Phase::Identify {
                return Err("Not ready to save.".into());
            }
            let r = g.ready.take().ok_or("Not ready to save.")?;
            *g = Inner::default();
            r
        };
        // The runtime picks the new file up in plan 4 task 7.
        if let Err(e) = strip_store::save(path, &ready.cfg) {
            log::warn!("strip file not saved: {e}");
            emit(WizardState::Failed {
                step: Step::Save,
                message: SAVE_FAILED.into(),
            });
            return Err(SAVE_FAILED.into());
        }
        emit(WizardState::Saved {
            device: ready.device,
            segments: ready.cfg.segments,
        });
        Ok(())
    }
}

async fn lan_phase<L: Lan>(
    lan: &L,
    device: DeviceDto,
    key: SecretString,
    emit: &Emit,
) -> Result<Ready, (Step, String)> {
    if !valid_device_id(&device.id) {
        return Err((Step::Search, "This device cannot be used.".into()));
    }
    let searching = |help| WizardState::Searching {
        device: device.clone(),
        since_us: now_us(),
        help,
    };
    emit(searching(false));
    let mut helped = false;
    let ip = loop {
        match lan.find(&device.id, FIND_WINDOW).await {
            Ok(ip) => break ip,
            Err(StripError::NotFound) => {
                if !helped {
                    helped = true;
                    emit(searching(true));
                }
                tokio::time::sleep(RETRY_PAUSE).await;
            }
            Err(e) => return Err((Step::Search, e.to_string())),
        }
    };
    let mut cfg = StripConfig {
        id: device.id.clone(),
        name: store_name(&device.name),
        local_key: key,
        segments: 10,
        last_ip: Some(ip),
    };
    emit(WizardState::Listening {
        device: device.clone(),
        since_us: now_us(),
    });
    let segments = match lan.listen(&cfg, ip, LISTEN_WINDOW).await {
        Ok(n) if (1..=60).contains(&n) => n,
        Ok(_) | Err(StripError::NoSegments) => return Err((Step::Segments, NO_PAINT.into())),
        Err(e) => return Err((Step::Segments, e.to_string())),
    };
    cfg.segments = segments;
    let conn = lan
        .connect(&cfg)
        .await
        .map_err(|e| (Step::Segments, e.to_string()))?;
    emit(WizardState::Identify {
        device: device.clone(),
        segments,
    });
    Ok(Ready { cfg, device, conn })
}

/// The real strip on the LAN.
pub struct RealLan;

struct RealConn(Strip);

impl Identify for RealConn {
    fn identify(&self, segment: u8, segments: u8) -> BoxFut<'_, Result<(), StripError>> {
        Box::pin(self.0.identify(segment, segments))
    }
}

impl Lan for RealLan {
    async fn find(&self, id: &str, window: Duration) -> Result<Ipv4Addr, StripError> {
        strip::find_by_id(id, None, window).await.map(|f| f.ip)
    }

    async fn listen(
        &self,
        cfg: &StripConfig,
        ip: Ipv4Addr,
        window: Duration,
    ) -> Result<u8, StripError> {
        strip::listen_segment_count(cfg, ip, window).await
    }

    async fn connect(&self, cfg: &StripConfig) -> Result<Arc<dyn Identify>, StripError> {
        Ok(Arc::new(RealConn(Strip::connect(cfg).await?)))
    }
}

#[cfg(test)]
#[allow(clippy::unused_async_trait_impl)] // the fakes answer at once; the real strip does not
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::collections::VecDeque;
    use std::os::unix::fs::PermissionsExt;
    use tuya_cloud::{CloudError, HttpRequest};

    const KEY: &str = "LOCALKEYAAAA1111";

    fn dev() -> DeviceDto {
        DeviceDto {
            id: "bf1234".into(),
            name: "Strip".into(),
            product: "P".into(),
            online: true,
            supported: true,
        }
    }

    type Log = Arc<Mutex<Vec<WizardState>>>;

    fn recorder() -> (Emit, Log) {
        let log: Log = Arc::default();
        let l = log.clone();
        let emit: Emit = Arc::new(move |s| l.lock().unwrap().push(s));
        (emit, log)
    }

    #[derive(Default)]
    struct FakeLan {
        finds: Mutex<VecDeque<Result<Ipv4Addr, StripError>>>,
        listen: Mutex<Option<Result<u8, StripError>>>,
        windows: Mutex<Vec<Duration>>,
        lit: Arc<Mutex<Vec<(u8, u8)>>>,
        connect_fails: bool,
    }

    struct FakeConn(Arc<Mutex<Vec<(u8, u8)>>>);
    impl Identify for FakeConn {
        fn identify(&self, segment: u8, segments: u8) -> BoxFut<'_, Result<(), StripError>> {
            self.0.lock().unwrap().push((segment, segments));
            Box::pin(async { Ok(()) })
        }
    }

    impl Lan for FakeLan {
        async fn find(&self, _: &str, window: Duration) -> Result<Ipv4Addr, StripError> {
            self.windows.lock().unwrap().push(window);
            self.finds
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(StripError::NotFound))
        }
        async fn listen(
            &self,
            _: &StripConfig,
            _: Ipv4Addr,
            window: Duration,
        ) -> Result<u8, StripError> {
            self.windows.lock().unwrap().push(window);
            self.listen
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Err(StripError::NoSegments))
        }
        async fn connect(&self, _: &StripConfig) -> Result<Arc<dyn Identify>, StripError> {
            if self.connect_fails {
                return Err(StripError::Connect);
            }
            Ok(Arc::new(FakeConn(self.lit.clone())))
        }
    }

    fn lan(finds: Vec<Result<Ipv4Addr, StripError>>, n: Result<u8, StripError>) -> FakeLan {
        FakeLan {
            finds: Mutex::new(finds.into()),
            listen: Mutex::new(Some(n)),
            ..FakeLan::default()
        }
    }

    const IP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 9);

    #[tokio::test(start_paused = true)]
    async fn searching_shows_help_after_first_miss_and_keeps_going() {
        let l = lan(
            vec![Err(StripError::NotFound), Err(StripError::NotFound), Ok(IP)],
            Ok(10),
        );
        let (emit, log) = recorder();
        let r = lan_phase(&l, dev(), SecretString::from(KEY), &emit)
            .await
            .unwrap();
        assert_eq!(r.cfg.segments, 10);
        assert_eq!(r.cfg.last_ip, Some(IP));
        let kinds: Vec<_> = log
            .lock()
            .unwrap()
            .iter()
            .map(|s| match s {
                WizardState::Searching { help, .. } => format!("search:{help}"),
                WizardState::Listening { .. } => "listen".into(),
                WizardState::Identify { segments, .. } => format!("identify:{segments}"),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            ["search:false", "search:true", "listen", "identify:10"]
        );
        assert_eq!(
            *l.windows.lock().unwrap(),
            [FIND_WINDOW, FIND_WINDOW, FIND_WINDOW, LISTEN_WINDOW]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn silent_strip_fails_with_the_compatibility_message() {
        let l = lan(vec![Ok(IP)], Err(StripError::NoSegments));
        let (emit, _) = recorder();
        let e = lan_phase(&l, dev(), SecretString::from(KEY), &emit)
            .await
            .err()
            .unwrap();
        assert_eq!(e, (Step::Segments, NO_PAINT.to_owned()));
    }

    #[tokio::test(start_paused = true)]
    async fn connect_failure_is_reported_on_the_segments_step() {
        let mut l = lan(vec![Ok(IP)], Ok(4));
        l.connect_fails = true;
        let (emit, _) = recorder();
        let e = lan_phase(&l, dev(), SecretString::from(KEY), &emit)
            .await
            .err()
            .unwrap();
        assert_eq!(e.0, Step::Segments);
    }

    #[tokio::test(start_paused = true)]
    async fn bad_cloud_id_or_name_cannot_reach_the_store() {
        let l = lan(vec![Ok(IP)], Ok(4));
        let (emit, _) = recorder();
        let mut d = dev();
        d.id = "bad id!".into();
        assert!(
            lan_phase(&l, d, SecretString::from(KEY), &emit)
                .await
                .is_err()
        );
        let mut d = dev();
        d.name = format!("a\u{7}{}", "x".repeat(100));
        let r = lan_phase(&l, d, SecretString::from(KEY), &emit)
            .await
            .unwrap();
        assert_eq!(r.cfg.name.chars().count(), 63);
        assert!(!r.cfg.name.contains('\u{7}'));
    }

    async fn at_identify(w: &Wizard, l: &FakeLan) -> (Emit, Log) {
        let (emit, log) = recorder();
        let r = lan_phase(l, dev(), SecretString::from(KEY), &emit)
            .await
            .unwrap();
        let mut g = lock(&w.0);
        g.phase = Phase::Identify;
        g.ready = Some(r);
        drop(g);
        (emit, log)
    }

    #[tokio::test(start_paused = true)]
    async fn identify_validates_the_segment_and_forwards_it() {
        let w = Wizard::default();
        let l = lan(vec![Ok(IP)], Ok(3));
        at_identify(&w, &l).await;
        for bad in [0u8, 4, 255] {
            assert!(w.identify(bad).await.is_err(), "{bad}");
        }
        assert!(l.lit.lock().unwrap().is_empty());
        assert!(w.identify(2).await.is_ok());
        assert_eq!(*l.lit.lock().unwrap(), [(2, 3)]);
    }

    #[tokio::test]
    async fn identify_outside_the_identify_step_is_refused() {
        assert!(Wizard::default().identify(1).await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn save_writes_a_0600_file_and_emits_saved() {
        let w = Wizard::default();
        let l = lan(vec![Ok(IP)], Ok(3));
        let (emit, log) = at_identify(&w, &l).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("strip.json");
        w.save(&path, &emit).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let cfg = strip_store::load(&path).unwrap();
        assert_eq!((cfg.segments, cfg.last_ip), (3, Some(IP)));
        assert_eq!(cfg.local_key.expose_secret(), KEY);
        assert!(matches!(
            log.lock().unwrap().last(),
            Some(WizardState::Saved { segments: 3, .. })
        ));
        // Done: a second save has nothing to write.
        assert!(w.save(&path, &emit).is_err());
    }

    #[tokio::test]
    async fn save_outside_the_identify_step_writes_nothing() {
        let (emit, _) = recorder();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("strip.json");
        assert!(Wizard::default().save(&path, &emit).is_err());
        assert!(!path.exists());
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_at_identify_drops_everything_and_goes_idle() {
        let w = Wizard::default();
        let l = lan(vec![Ok(IP)], Ok(3));
        let (emit, log) = at_identify(&w, &l).await;
        w.cancel(&emit);
        assert!(lock(&w.0).ready.is_none());
        assert_eq!(log.lock().unwrap().last(), Some(&WizardState::Idle));
        assert!(w.identify(1).await.is_err());
    }

    #[test]
    fn choose_rejects_malformed_ids_and_idle_wizard() {
        let w = Wizard::default();
        for bad in ["", "a b", "x/../y", &"a".repeat(65), "é"] {
            assert!(w.choose(bad).is_err(), "{bad:?}");
        }
        assert!(w.choose("bf1234").is_err(), "no run is waiting");
    }

    struct Down;
    impl Transport for Down {
        fn send(&self, _: &HttpRequest) -> Result<String, CloudError> {
            Err(CloudError::Network)
        }
    }

    #[tokio::test]
    async fn start_twice_is_refused_and_a_failed_run_frees_the_wizard() {
        let w = Wizard::default();
        let (emit, log) = recorder();
        let go = |w: &Wizard| {
            w.start(
                Arc::new(FakeLan::default()),
                Arc::new(Down),
                UserCode::parse("abc123").unwrap(),
                emit.clone(),
            )
        };
        assert!(go(&w).is_ok());
        assert!(go(&w).is_err());
        for _ in 0..100 {
            if lock(&w.0).phase == Phase::Idle {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(lock(&w.0).phase, Phase::Idle);
        assert!(matches!(
            log.lock().unwrap().last(),
            Some(WizardState::Failed {
                step: Step::Login,
                ..
            })
        ));
        assert!(go(&w).is_ok(), "a new run can start");
    }

    #[tokio::test(start_paused = true)]
    async fn emitted_states_never_carry_the_key() {
        let l = lan(vec![Ok(IP)], Ok(5));
        let (emit, log) = recorder();
        lan_phase(&l, dev(), SecretString::from(KEY), &emit)
            .await
            .unwrap();
        let json = serde_json::to_string(&*log.lock().unwrap()).unwrap();
        assert!(!json.contains(KEY));
    }
}
