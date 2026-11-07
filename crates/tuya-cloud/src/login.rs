//! Smart Life QR "device sharing" login: scan, read the device list, revoke.
//!
//! No token refresh: a session lives for seconds here and is always revoked via [`run_and_revoke`].
//! Decrypted plaintext is held in `Zeroizing` and the device list is parsed straight into
//! `SecretString` fields, so no long-lived plain copy of a key is made.

use crate::CloudError;
use crate::crypto;
use crate::transport::{HttpRequest, Method, Transport};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt::Write as _;
use zeroize::Zeroizing;

pub const CLIENT_ID: &str = "HA_3y9q4ak7g4ephrvke";
pub const SCHEMA: &str = "haauthorize";
pub const LOGIN_HOST: &str = "apigw.iotbing.com";
/// Extend only with a host actually observed in a real login.
pub const ENDPOINT_HOST_SUFFIXES: &[&str] = &[
    "iotbing.com",
    "tuyaeu.com",
    "tuyaus.com",
    "tuyacn.com",
    "tuyain.com",
];

const TOKENS_PATH: &str = "/v1.0/m/life/home-assistant/qrcode/tokens";

pub struct UserCode(SecretString);

impl UserCode {
    /// # Errors
    /// `InvalidUserCode` unless the trimmed input is 1-32 ASCII alphanumerics.
    pub fn parse(s: &str) -> Result<Self, CloudError> {
        let s = s.trim();
        if (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric()) {
            Ok(Self(s.to_owned().into()))
        } else {
            Err(CloudError::InvalidUserCode)
        }
    }
}

impl std::fmt::Debug for UserCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UserCode([REDACTED])")
    }
}

pub struct QrLogin {
    pub token: SecretString,
    pub qr_payload: SecretString,
}

impl QrLogin {
    #[cfg(test)]
    pub(crate) fn for_token(token: &str) -> Self {
        Self::new(token.to_owned())
    }

    fn new(token: String) -> Self {
        let qr_payload = format!("tuyaSmart--qrLogin?token={token}").into();
        Self {
            token: token.into(),
            qr_payload,
        }
    }
}

impl std::fmt::Debug for QrLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QrLogin([REDACTED])")
    }
}

pub struct Session {
    endpoint: String,
    terminal_id: SecretString,
    access_token: SecretString,
    refresh_token: SecretString,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session {{ endpoint: {:?}, secrets: [REDACTED] }}",
            self.endpoint
        )
    }
}

impl Session {
    /// Host of the validated endpoint (not a secret).
    #[must_use]
    pub fn endpoint_host(&self) -> &str {
        self.endpoint
            .strip_prefix("https://")
            .unwrap_or(&self.endpoint)
    }
}

#[derive(Deserialize)]
pub struct CloudDevice {
    pub id: String,
    pub name: String,
    pub category: String,
    pub product_name: String,
    pub online: bool,
    pub local_key: SecretString,
}

impl CloudDevice {
    #[must_use]
    pub fn is_light_strip(&self) -> bool {
        self.category == "dd"
    }
}

impl std::fmt::Debug for CloudDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudDevice")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("category", &self.category)
            .field("product_name", &self.product_name)
            .field("online", &self.online)
            .field("local_key", &"[REDACTED]")
            .finish()
    }
}

fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn api_code(v: &Value) -> String {
    match &v["code"] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => "unknown".to_owned(),
    }
}

fn parse_json(body: &str) -> Result<Value, CloudError> {
    serde_json::from_str(body).map_err(|_| CloudError::Decode)
}

fn str_field(v: &Value, key: &str) -> Result<String, CloudError> {
    match &v[key] {
        Value::String(s) if !s.is_empty() => Ok(s.clone()),
        _ => Err(CloudError::Decode),
    }
}

/// Starts a login; show `qr_payload` as a QR code for the user to scan in Smart Life.
///
/// # Errors
/// Transport errors, `Api` when the cloud refuses, `Decode` on a malformed answer.
pub fn start_login(t: &dyn Transport, code: &UserCode) -> Result<QrLogin, CloudError> {
    let url = format!(
        "https://{LOGIN_HOST}{TOKENS_PATH}?clientid={CLIENT_ID}&usercode={}&schema={SCHEMA}",
        code.0.expose_secret()
    );
    let v = parse_json(&t.send(&HttpRequest {
        method: Method::Post,
        url,
        headers: vec![],
        body: None,
    })?)?;
    if v["success"] != true {
        return Err(CloudError::Api { code: api_code(&v) });
    }
    let token = str_field(&v["result"], "qrcode")?;
    // The token goes into a URL path later: refuse anything that is not plain token characters.
    if !token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(CloudError::Decode);
    }
    Ok(QrLogin::new(token))
}

/// One poll request (cadence and deadline are the caller's). `None` until the user confirmed.
///
/// # Errors
/// Transport errors, `Decode`, and `EndpointNotAllowed` (the session is dropped, nothing more is sent).
pub fn poll_login(
    t: &dyn Transport,
    code: &UserCode,
    qr: &QrLogin,
) -> Result<Option<Session>, CloudError> {
    let url = format!(
        "https://{LOGIN_HOST}{TOKENS_PATH}/{}?clientid={CLIENT_ID}&usercode={}",
        qr.token.expose_secret(),
        code.0.expose_secret()
    );
    let v = parse_json(&t.send(&HttpRequest {
        method: Method::Get,
        url,
        headers: vec![],
        body: None,
    })?)?;
    if v["success"] != true {
        return Ok(None);
    }
    let r = &v["result"];
    let endpoint = validate_endpoint(r["endpoint"].as_str().ok_or(CloudError::Decode)?)?;
    Ok(Some(Session {
        endpoint,
        terminal_id: str_field(r, "terminal_id")?.into(),
        access_token: str_field(r, "access_token")?.into(),
        refresh_token: str_field(r, "refresh_token")?.into(),
    }))
}

/// Accepts exactly `https://<host>[/]` with `host` equal to or a subdomain of an allowlisted suffix.
fn validate_endpoint(raw: &str) -> Result<String, CloudError> {
    let rest = raw
        .strip_prefix("https://")
        .ok_or(CloudError::EndpointNotAllowed)?;
    let host = rest.strip_suffix('/').unwrap_or(rest).to_ascii_lowercase();
    let well_formed = !host.is_empty()
        && host
            .split('.')
            .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    let allowed = ENDPOINT_HOST_SUFFIXES
        .iter()
        .any(|s| host == *s || host.ends_with(&format!(".{s}")));
    if well_formed && allowed {
        Ok(format!("https://{host}"))
    } else {
        Err(CloudError::EndpointNotAllowed)
    }
}

fn uuid_v4() -> Result<String, CloudError> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| CloudError::Crypto)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    });
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}

fn now_ms() -> Result<String, CloudError> {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| CloudError::Crypto)?;
    Ok(d.as_millis().to_string())
}

/// Encrypted, signed call. `payload` is the query params (GET) or body (POST) object.
fn signed_call(
    t: &dyn Transport,
    s: &Session,
    method: Method,
    path: &str,
    payload: Option<&Value>,
) -> Result<Zeroizing<String>, CloudError> {
    let rid = uuid_v4()?;
    let hash_key = crypto::hash_key(&rid, s.refresh_token.expose_secret());
    let secret = crypto::secret(&rid, &hash_key);
    let enc = match payload {
        Some(p) => crypto::encrypt_request(
            &Zeroizing::new(p.to_string()),
            &secret,
            &crypto::random_nonce()?,
        )?,
        None => String::new(),
    };
    let (query_enc, body_enc) = match method {
        Method::Get => (enc.as_str(), ""),
        Method::Post => ("", enc.as_str()),
    };
    let time = now_ms()?;
    let token = s.access_token.expose_secret();
    let sign = crypto::sign(
        &hash_key,
        &[
            ("X-appKey", CLIENT_ID),
            ("X-requestId", &rid),
            ("X-sid", ""),
            ("X-time", &time),
            ("X-token", token),
        ],
        query_enc,
        body_enc,
    );
    let mut url = format!("{}{path}", s.endpoint);
    if !query_enc.is_empty() {
        url.push_str("?encdata=");
        url.push_str(&pct_encode(query_enc));
    }
    let mut headers: Vec<(String, String)> = [
        ("X-appKey", CLIENT_ID),
        ("X-requestId", &rid),
        ("X-sid", ""),
        ("X-time", &time),
        ("X-token", token),
        ("X-sign", &sign),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let body = if body_enc.is_empty() {
        None
    } else {
        headers.push(("Content-Type".into(), "application/json".into()));
        Some(json!({ "encdata": body_enc }).to_string())
    };
    let v = parse_json(&t.send(&HttpRequest {
        method,
        url,
        headers,
        body,
    })?)?;
    if v["success"] != true {
        return Err(CloudError::Api { code: api_code(&v) });
    }
    match &v["result"] {
        Value::String(enc) => crypto::decrypt_response(enc, &secret),
        _ => Ok(Zeroizing::new("null".to_owned())),
    }
}

/// Lists the devices of every home of the account.
///
/// # Errors
/// Transport, `Api`, `Crypto`, or `Decode` (including a device without its required fields).
pub fn list_devices(t: &dyn Transport, s: &Session) -> Result<Vec<CloudDevice>, CloudError> {
    let homes = parse_json(&signed_call(
        t,
        s,
        Method::Get,
        "/v1.0/m/life/users/homes",
        None,
    )?)?;
    let mut out = Vec::new();
    for home in homes.as_array().ok_or(CloudError::Decode)? {
        let home_id = match &home["ownerId"] {
            Value::String(x) if !x.is_empty() => x.clone(),
            Value::Number(n) => n.to_string(),
            _ => return Err(CloudError::Decode),
        };
        let devs = signed_call(
            t,
            s,
            Method::Get,
            "/v1.0/m/life/ha/home/devices",
            Some(&json!({ "homeId": home_id })),
        )?;
        let devs: Vec<CloudDevice> = serde_json::from_str(&devs).map_err(|_| CloudError::Decode)?;
        out.extend(devs);
    }
    Ok(out)
}

/// Revokes the session on the cloud side; consumes it so it cannot be reused.
///
/// # Errors
/// Transport, `Api`, or `Crypto` errors; the caller should tell the user the session may still be live.
#[allow(clippy::needless_pass_by_value)] // taking the session by value is the point: no reuse after revoke
pub fn revoke(t: &dyn Transport, s: Session) -> Result<(), CloudError> {
    let body = json!({
        "accessToken": s.access_token.expose_secret(),
        "terminalId": s.terminal_id.expose_secret(),
    });
    signed_call(
        t,
        &s,
        Method::Post,
        "/v1.0/m/token/terminal/expire",
        Some(&body),
    )
    .map(|_| ())
}

/// Runs `f`, then always revokes. A failed revoke is reported even if `f` succeeded
/// (the caller then knows the session may be alive); `f`'s error wins otherwise.
///
/// # Errors
/// Whatever `f` or `revoke` returned.
pub fn run_and_revoke<T>(
    t: &dyn Transport,
    s: Session,
    f: impl FnOnce(&dyn Transport, &Session) -> Result<T, CloudError>,
) -> Result<T, CloudError> {
    let result = f(t, &s);
    let revoked = revoke(t, s);
    let value = result?;
    revoked.map(|()| value)
}
