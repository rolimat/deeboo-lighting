//! Smart Life cloud steps: QR login, device list, revoke, and the user's pick.
//!
//! Everything blocking runs in `spawn_blocking`. The cloud session only ever lives inside this
//! function: it is revoked right after the device list, and at once if the user cancels while a
//! login request is in flight. Only the chosen strip's key leaves; all other keys are dropped here.

use crate::qr::qr_matrix;
use crate::wizard::state::{DeviceDto, Step, WizardState};
use secrecy::SecretString;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{Receiver, error::TryRecvError};
use tokio::time::{Instant, timeout};
use tuya_cloud::{
    CloudDevice, CloudError, HttpRequest, QrLogin, Session, Transport, UserCode, list_devices,
    poll_login, revoke, start_login,
};

const POLL_EVERY: Duration = Duration::from_secs(2);
const LOGIN_WINDOW: Duration = Duration::from_secs(300);
/// Keys of every listed device sit in memory while the user chooses.
const CHOOSE_WINDOW: Duration = Duration::from_secs(600);

/// What the UI can tell the running flow. A dropped sender counts as `Cancel` (window closed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Choose(String),
    Cancel,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WizardError {
    Cancelled,
    TimedOut,
    ChooseTimedOut,
    Qr,
    Cloud(CloudError),
}

impl std::fmt::Display for WizardError {
    /// UI-safe copy; never carries cloud details.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Cancelled.",
            Self::TimedOut => "The QR code expired. Start again.",
            Self::ChooseTimedOut => "Took too long. Start again.",
            Self::Qr => "Could not build the QR code.",
            Self::Cloud(_) => {
                "Could not reach Smart Life. Check the User Code and your connection, then try again."
            }
        })
    }
}

type Transp = Arc<dyn Transport + Send + Sync>;

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_micros()).ok())
        .unwrap_or(0)
}

/// After a cancel only the revoke may still go out: the rest of the device list is skipped, so the
/// session is closed after at most the one request already in flight.
struct Abortable {
    inner: Transp,
    abort: Arc<AtomicBool>,
}

impl Transport for Abortable {
    fn send(&self, req: &HttpRequest) -> Result<String, CloudError> {
        if self.abort.load(Ordering::SeqCst) && !req.url.contains("/terminal/expire") {
            return Err(CloudError::Network);
        }
        self.inner.send(req)
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CloudError> + Send + 'static,
) -> Result<T, WizardError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| WizardError::Cloud(CloudError::Network))?
        .map_err(WizardError::Cloud)
}

/// Runs the cloud half of the wizard and returns the chosen strip with its key.
///
/// # Errors
/// `Cancelled` (also when `inputs` closes), `TimedOut` after 5 minutes without a scan, or the cloud
/// failure. Failures are also emitted as `WizardState::Failed`, cancellation as `Idle`.
pub async fn run(
    transport: Transp,
    code: UserCode,
    events: impl Fn(WizardState) + Send + Sync,
    mut inputs: Receiver<Input>,
) -> Result<(DeviceDto, SecretString), WizardError> {
    let result = flow(&transport, code, &events, &mut inputs).await;
    match &result {
        Ok(_) => {}
        Err((_, WizardError::Cancelled)) => events(WizardState::Idle),
        Err((step, e)) => {
            if let WizardError::Cloud(c) = e {
                log::debug!("wizard: cloud step failed: {c}");
            }
            events(WizardState::Failed {
                step: *step,
                message: e.to_string(),
            });
        }
    }
    result.map_err(|(_, e)| e)
}

/// Lists the devices, then revokes `session`; a cancel meanwhile ends the list early (see `Abortable`).
async fn list_and_revoke(
    transport: &Transp,
    session: Session,
    inputs: &mut Receiver<Input>,
) -> Result<(Vec<CloudDevice>, bool), (Step, WizardError)> {
    let devices = |e| (Step::Devices, e);
    let abort = Arc::new(AtomicBool::new(false));
    let t = Abortable {
        inner: transport.clone(),
        abort: abort.clone(),
    };
    // The session is consumed by revoke whatever happens; a failed revoke must not block the user.
    let mut job = tokio::task::spawn_blocking(move || {
        let listed = list_devices(&t, &session);
        let revoked = revoke(&t, session).is_ok();
        listed.map(|l| (l, revoked))
    });
    let mut cancelled = false;
    let joined = loop {
        tokio::select! {
            r = &mut job => break r,
            i = inputs.recv(), if !cancelled => {
                if matches!(i, Some(Input::Cancel) | None) {
                    cancelled = true;
                    abort.store(true, Ordering::SeqCst);
                }
            }
        }
    };
    let result = joined.map_err(|_| devices(WizardError::Cloud(CloudError::Network)))?;
    if cancelled {
        log::debug!(
            "wizard: cancelled during the device list, revoked={}",
            result.as_ref().is_ok_and(|r| r.1)
        );
        return Err(devices(WizardError::Cancelled));
    }
    result.map_err(|e| devices(WizardError::Cloud(e)))
}

async fn flow(
    transport: &Transp,
    code: UserCode,
    events: &(impl Fn(WizardState) + Send + Sync),
    inputs: &mut Receiver<Input>,
) -> Result<(DeviceDto, SecretString), (Step, WizardError)> {
    let login = |e| (Step::Login, e);
    let code = Arc::new(code);
    let t = transport.clone();
    let c = code.clone();
    let qr: Arc<QrLogin> = Arc::new(
        blocking(move || start_login(&*t, &c))
            .await
            .map_err(login)?,
    );
    log::debug!("wizard: login started");
    let matrix = qr_matrix(&qr.qr_payload).map_err(|_| login(WizardError::Qr))?;
    events(WizardState::WaitingScan {
        qr: matrix,
        expires_at_us: now_us() + LOGIN_WINDOW.as_secs() * 1_000_000,
    });

    let deadline = Instant::now() + LOGIN_WINDOW;
    let session = loop {
        if Instant::now() >= deadline {
            return Err(login(WizardError::TimedOut));
        }
        // The wait doubles as the cancel check: no timer thread, no flag polling.
        match timeout(POLL_EVERY, inputs.recv()).await {
            Ok(Some(Input::Cancel) | None) => return Err(login(WizardError::Cancelled)),
            Ok(Some(Input::Choose(_))) => continue,
            Err(_elapsed) => {}
        }
        let (t, c, q) = (transport.clone(), code.clone(), qr.clone());
        if let Some(s) = blocking(move || poll_login(&*t, &c, &q))
            .await
            .map_err(login)?
        {
            break s;
        }
    };
    drop(qr);

    // A cancel that arrived while the request was in flight: close the session before anything else.
    if matches!(
        inputs.try_recv(),
        Ok(Input::Cancel) | Err(TryRecvError::Disconnected)
    ) {
        let t = transport.clone();
        let revoked = blocking(move || revoke(&*t, session)).await;
        log::debug!("wizard: cancelled, revoked={}", revoked.is_ok());
        return Err(login(WizardError::Cancelled));
    }

    let devices = |e| (Step::Devices, e);
    let (listed, revoked) = list_and_revoke(transport, session, inputs).await?;
    if revoked {
        log::debug!("wizard: {} devices listed, session revoked", listed.len());
    } else {
        log::warn!("cloud session could not be revoked; it expires on its own");
    }

    let mut entries: Vec<(DeviceDto, SecretString)> = listed
        .into_iter()
        .map(|d| {
            let usable = d.local_key.is_some();
            let dto = DeviceDto {
                supported: d.is_light_strip() && usable,
                reason: (!usable).then(|| "Not usable: no local key".to_owned()),
                id: d.id,
                name: d.name,
                product: d.product_name,
                online: d.online,
            };
            // Unusable devices carry an empty placeholder: they can never be chosen.
            (
                dto,
                d.local_key
                    .unwrap_or_else(|| SecretString::from(String::new())),
            )
        })
        .collect();
    events(WizardState::ChooseStrip {
        devices: entries.iter().map(|(d, _)| d.clone()).collect(),
    });
    loop {
        let input = timeout(CHOOSE_WINDOW, inputs.recv())
            .await
            .map_err(|_| devices(WizardError::ChooseTimedOut))?;
        match input {
            Some(Input::Choose(id)) => {
                if let Some(i) = entries.iter().position(|(d, _)| d.id == id && d.supported) {
                    // `entries` drops on return: every other key is wiped (SecretString) right here.
                    return Ok(entries.swap_remove(i));
                }
            }
            Some(Input::Cancel) | None => return Err(devices(WizardError::Cancelled)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wizard::state::WizardState as S;
    use secrecy::ExposeSecret;
    use std::fmt::Write as _;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc::{Sender, channel};
    use tuya_cloud::testing::encrypted_reply;
    use tuya_cloud::{HttpRequest, Method};

    const CODE: &str = "UCode9z";
    const TOKEN: &str = "TKN-SECRET-77";
    const ACCESS: &str = "at-SECRET";
    const REFRESH: &str = "rt-SECRET";
    const KEY_A: &str = "LOCALKEYAAAA1111";
    const KEY_B: &str = "LOCALKEYBBBB2222";
    const SECRETS: [&str; 6] = [CODE, TOKEN, ACCESS, REFRESH, KEY_A, KEY_B];

    struct Down;
    impl Transport for Down {
        fn send(&self, _: &HttpRequest) -> Result<String, CloudError> {
            Err(CloudError::Network)
        }
    }

    #[derive(Default)]
    struct Fake {
        seen: Mutex<Vec<(Method, String)>>,
        not_yet: AtomicUsize,
        never: bool,
        fail_revoke: bool,
        on_poll: Mutex<Option<Box<dyn Fn() + Send>>>,
    }

    impl Fake {
        fn urls(&self) -> Vec<String> {
            self.seen
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.1.clone())
                .collect()
        }
        fn count(&self, needle: &str) -> usize {
            self.urls().iter().filter(|u| u.contains(needle)).count()
        }
    }

    impl Transport for Fake {
        fn send(&self, req: &HttpRequest) -> Result<String, CloudError> {
            self.seen
                .lock()
                .unwrap()
                .push((req.method, req.url.clone()));
            let u = &req.url;
            if u.contains("/qrcode/tokens/") {
                if let Some(f) = self.on_poll.lock().unwrap().as_ref() {
                    f();
                }
                if self.never || self.not_yet.load(Ordering::SeqCst) > 0 {
                    self.not_yet.fetch_sub(1, Ordering::SeqCst);
                    return Ok(r#"{"success":false}"#.into());
                }
                Ok(serde_json::json!({"success": true, "result": {
                    "access_token": ACCESS, "refresh_token": REFRESH, "uid": "u1",
                    "expire_time": 7200, "terminal_id": "term1",
                    "endpoint": "https://apigw.tuyaeu.com", "username": "x"}})
                .to_string())
            } else if u.contains("/qrcode/tokens") {
                Ok(format!(
                    r#"{{"success":true,"result":{{"qrcode":"{TOKEN}"}}}}"#
                ))
            } else if u.contains("/users/homes") {
                Ok(encrypted_reply(req, REFRESH, r#"[{"ownerId":"1"}]"#))
            } else if u.contains("/ha/home/devices") {
                let devs = format!(
                    r#"[{{"id":"d1","name":"Strip","category":"dd","product_name":"JSK","online":true,"local_key":"{KEY_A}"}},
                        {{"id":"d2","name":"Bulb","category":"dj","product_name":"B","online":false,"local_key":"{KEY_B}"}}]"#
                );
                Ok(encrypted_reply(req, REFRESH, &devs))
            } else if u.contains("/terminal/expire") {
                if self.fail_revoke {
                    return Err(CloudError::Http(500));
                }
                Ok(encrypted_reply(req, REFRESH, "true"))
            } else {
                Err(CloudError::Network)
            }
        }
    }

    type Events = Arc<Mutex<Vec<WizardState>>>;

    /// Runs the flow in paused virtual time. `choose` is sent when the list is shown.
    fn drive(
        fake: &Arc<Fake>,
        choose: &[&str],
        pre: impl FnOnce(&Sender<Input>),
    ) -> (Result<(DeviceDto, SecretString), WizardError>, Events) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        let (tx, rx) = channel(8);
        pre(&tx);
        let events: Events = Arc::default();
        let (ev, tx2) = (events.clone(), tx.clone());
        let picks: Vec<String> = choose.iter().map(|s| (*s).to_owned()).collect();
        let on_event = move |s: WizardState| {
            if matches!(s, S::ChooseStrip { .. }) {
                for p in &picks {
                    tx2.try_send(Input::Choose(p.clone())).unwrap();
                }
            }
            ev.lock().unwrap().push(s);
        };
        let t: Arc<dyn Transport + Send + Sync> = fake.clone();
        let out = rt.block_on(run(t, UserCode::parse(CODE).unwrap(), on_event, rx));
        (out, events)
    }

    fn fake() -> Arc<Fake> {
        Arc::new(Fake::default())
    }

    #[test]
    fn happy_path_emits_qr_then_devices_and_revokes() {
        let f = fake();
        f.not_yet.store(2, Ordering::SeqCst);
        let (out, ev) = drive(&f, &["d1"], |_| {});
        let (dev, key) = out.unwrap();
        assert_eq!((dev.id.as_str(), dev.supported), ("d1", true));
        assert_eq!(key.expose_secret(), KEY_A);
        let ev = ev.lock().unwrap();
        assert!(matches!(&ev[0], S::WaitingScan { qr, .. } if qr.size > 0));
        assert!(matches!(&ev[1], S::ChooseStrip { devices } if devices.len() == 2));
        assert_eq!(ev.len(), 2);
        assert_eq!(f.count("/qrcode/tokens/"), 3);
        assert_eq!(f.count("/terminal/expire"), 1);
        let urls = f.urls();
        assert!(urls.last().unwrap().ends_with("/terminal/expire"));
    }

    #[test]
    fn cancel_revokes_and_forgets() {
        let f = fake();
        // The user cancels while the poll that returns the session is in flight.
        let (tx, rx) = channel(8);
        *f.on_poll.lock().unwrap() = Some({
            let tx = tx.clone();
            Box::new(move || tx.try_send(Input::Cancel).unwrap())
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        let t: Arc<dyn Transport + Send + Sync> = f.clone();
        let out = rt.block_on(run(t, UserCode::parse(CODE).unwrap(), |_| {}, rx));
        assert_eq!(out.err(), Some(WizardError::Cancelled));
        assert_eq!(f.count("/terminal/expire"), 1);
        assert_eq!(f.count("/ha/home/devices"), 0);
    }

    #[test]
    fn cancel_before_scan_sends_nothing_more() {
        let f = fake();
        let (out, ev) = drive(&f, &[], |tx| tx.try_send(Input::Cancel).unwrap());
        assert_eq!(out.err(), Some(WizardError::Cancelled));
        assert_eq!(f.urls().len(), 1);
        assert_eq!(ev.lock().unwrap().last(), Some(&S::Idle));
    }

    #[test]
    fn dropping_the_input_channel_cancels() {
        let f = fake();
        let t: Arc<dyn Transport + Send + Sync> = f.clone();
        let (tx, rx) = channel(1);
        drop(tx);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        let out = rt.block_on(run(t, UserCode::parse(CODE).unwrap(), |_| {}, rx));
        assert_eq!(out.err(), Some(WizardError::Cancelled));
    }

    #[test]
    fn timeout_after_5_min_fails_and_has_no_session_to_revoke() {
        let f = Arc::new(Fake {
            never: true,
            ..Fake::default()
        });
        let (out, ev) = drive(&f, &[], |_| {});
        assert_eq!(out.err(), Some(WizardError::TimedOut));
        assert_eq!(f.count("/terminal/expire"), 0);
        let polls = f.count("/qrcode/tokens/");
        assert!((149..=150).contains(&polls), "{polls}");
        assert!(matches!(
            ev.lock().unwrap().last(),
            Some(S::Failed {
                step: Step::Login,
                ..
            })
        ));
    }

    #[test]
    fn unsupported_devices_listed_but_not_choosable() {
        let f = fake();
        let (out, ev) = drive(&f, &["d2", "nope", "d1"], |_| {});
        assert_eq!(out.unwrap().0.id, "d1");
        let ev = ev.lock().unwrap();
        let S::ChooseStrip { devices } = &ev[1] else {
            unreachable!()
        };
        assert_eq!(
            devices.iter().map(|d| d.supported).collect::<Vec<_>>(),
            [true, false]
        );
    }

    #[test]
    fn cloud_failure_is_reported_without_details() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        let (_tx, rx) = channel(1);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        let out = rt.block_on(run(
            Arc::new(Down),
            UserCode::parse(CODE).unwrap(),
            move |s| s2.lock().unwrap().push(s),
            rx,
        ));
        assert!(matches!(out, Err(WizardError::Cloud(_))));
        assert!(matches!(
            seen.lock().unwrap().last(),
            Some(S::Failed {
                step: Step::Login,
                ..
            })
        ));
    }

    #[test]
    fn device_dto_never_contains_key() {
        let f = fake();
        let (out, ev) = drive(&f, &["d1"], |_| {});
        let (dev, _) = out.unwrap();
        let mut all = serde_json::to_string(&dev).unwrap();
        for s in ev.lock().unwrap().iter() {
            all.push_str(&serde_json::to_string(s).unwrap());
            write!(all, "{s:?}").unwrap();
        }
        for k in [KEY_A, KEY_B, TOKEN, ACCESS, REFRESH, CODE] {
            assert!(!all.contains(k), "{k}");
        }
    }

    #[test]
    fn no_secret_in_logs_through_the_flow() {
        let _log = crate::wizard::testlog::begin();
        let f = fake();
        f.not_yet.store(1, Ordering::SeqCst);
        let (out, _) = drive(&f, &["d1"], |_| {});
        let err = format!("{:?}", out.as_ref().err());
        assert!(out.is_ok());
        let logs = crate::wizard::testlog::lines();
        assert!(!logs.is_empty(), "the flow should log its steps");
        for s in SECRETS {
            assert!(!logs.contains(s) && !err.contains(s), "{s}");
        }
    }

    #[test]
    fn revoke_failure_still_lists_devices_and_drops_session() {
        let _log = crate::wizard::testlog::begin();
        let f = Arc::new(Fake {
            fail_revoke: true,
            ..Fake::default()
        });
        let (out, ev) = drive(&f, &["d1"], |_| {});
        assert_eq!(out.unwrap().1.expose_secret(), KEY_A);
        assert_eq!(f.count("/terminal/expire"), 1);
        assert!(matches!(&ev.lock().unwrap()[1], S::ChooseStrip { .. }));
        let logs = crate::wizard::testlog::lines();
        assert!(!SECRETS.iter().any(|s| logs.contains(s)));
    }

    #[test]
    fn choose_wait_ends_after_10_minutes() {
        let f = fake();
        let (out, ev) = drive(&f, &[], |_| {});
        assert_eq!(out.err(), Some(WizardError::ChooseTimedOut));
        assert!(matches!(
            ev.lock().unwrap().last(),
            Some(S::Failed { step: Step::Devices, message }) if message == "Took too long. Start again."
        ));
    }
}
