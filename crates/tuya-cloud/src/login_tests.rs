use crate::CloudError;
use crate::crypto;
use crate::login::*;
use crate::transport::{HttpRequest, Method, Transport, UreqTransport};
use secrecy::ExposeSecret;
use std::cell::RefCell;

const REFRESH: &str = "rt-SECRET";
const ACCESS: &str = "at-SECRET";

type Handler = Box<dyn Fn(&HttpRequest) -> Result<String, CloudError>>;

struct Fake {
    seen: RefCell<Vec<HttpRequest>>,
    handler: Handler,
}

impl Fake {
    fn new(handler: impl Fn(&HttpRequest) -> Result<String, CloudError> + 'static) -> Self {
        Self {
            seen: RefCell::new(Vec::new()),
            handler: Box::new(handler),
        }
    }
    fn urls(&self) -> Vec<String> {
        self.seen.borrow().iter().map(|r| r.url.clone()).collect()
    }
}

impl Transport for Fake {
    fn send(&self, req: &HttpRequest) -> Result<String, CloudError> {
        self.seen.borrow_mut().push(req.clone());
        (self.handler)(req)
    }
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> &'a str {
    req.headers
        .iter()
        .find(|(k, _)| k == name)
        .map_or("", |(_, v)| v.as_str())
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

/// Raw (still encrypted) request payload: query `encdata` for GET, body `encdata` for POST.
fn raw_encdata(req: &HttpRequest) -> String {
    match req.method {
        Method::Get => req
            .url
            .split_once("?encdata=")
            .map(|(_, e)| pct_decode(e))
            .unwrap_or_default(),
        Method::Post => {
            let v: serde_json::Value = serde_json::from_str(req.body.as_ref().unwrap()).unwrap();
            v["encdata"].as_str().unwrap().to_owned()
        }
    }
}

fn secret_of(req: &HttpRequest) -> String {
    let rid = header(req, "X-requestId");
    crypto::secret(rid, &crypto::hash_key(rid, REFRESH))
}

fn reply(req: &HttpRequest, result: &str) -> String {
    let enc = crypto::encrypt_request(result, &secret_of(req), b"abcdefghijkl").unwrap();
    serde_json::json!({"success": true, "result": enc, "t": 1}).to_string()
}

fn login_ok(endpoint: &str) -> String {
    serde_json::json!({"success": true, "t": 1, "result": {
        "access_token": ACCESS, "refresh_token": REFRESH, "uid": "u1",
        "expire_time": 7200, "terminal_id": "term1", "endpoint": endpoint, "username": "x"
    }})
    .to_string()
}

fn code() -> UserCode {
    UserCode::parse("Abc123").unwrap()
}

fn qr() -> QrLogin {
    QrLogin::for_token("TKN")
}

fn session() -> Session {
    let f = Fake::new(|_| Ok(login_ok("https://apigw.tuyaeu.com")));
    poll_login(&f, &code(), &qr()).unwrap().unwrap()
}

#[test]
fn start_login_posts_exact_url_and_builds_qr_payload() {
    let f = Fake::new(|_| Ok(r#"{"success":true,"result":{"qrcode":"TKN"}}"#.into()));
    let qr = start_login(&f, &code()).unwrap();
    let seen = f.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, Method::Post);
    assert_eq!(
        seen[0].url,
        "https://apigw.iotbing.com/v1.0/m/life/home-assistant/qrcode/tokens?clientid=HA_3y9q4ak7g4ephrvke&usercode=Abc123&schema=haauthorize"
    );
    assert_eq!(qr.token.expose_secret(), "TKN");
    assert_eq!(
        qr.qr_payload.expose_secret(),
        "tuyaSmart--qrLogin?token=TKN"
    );
}

#[test]
fn rejects_user_code_with_url_characters() {
    for bad in [
        "ab&c",
        "a b",
        "",
        "   ",
        &"a".repeat(33),
        "ab/c",
        "ab?c",
        "é",
    ] {
        assert!(
            matches!(UserCode::parse(bad), Err(CloudError::InvalidUserCode)),
            "{bad}"
        );
    }
    assert!(UserCode::parse("  Abc123 ").is_ok());
    assert!(UserCode::parse(&"a".repeat(32)).is_ok());
}

#[test]
fn start_login_api_failure_and_bad_token() {
    let f = Fake::new(|_| Ok(r#"{"success":false,"code":1004,"msg":"x"}"#.into()));
    assert_eq!(
        start_login(&f, &code()).err(),
        Some(CloudError::Api {
            code: "1004".into()
        })
    );
    let f = Fake::new(|_| Ok(r#"{"success":true,"result":{"qrcode":"a b&c"}}"#.into()));
    assert_eq!(start_login(&f, &code()).err(), Some(CloudError::Decode));
}

#[test]
fn poll_returns_none_until_scanned_then_session() {
    let f = Fake::new(|r| {
        if r.url.contains("TKN") && r.method == Method::Get && header(r, "X-token").is_empty() {
            Ok(r#"{"success":false,"code":"x"}"#.into())
        } else {
            Err(CloudError::Network)
        }
    });
    assert!(poll_login(&f, &code(), &qr()).unwrap().is_none());
    assert_eq!(
        f.urls()[0],
        "https://apigw.iotbing.com/v1.0/m/life/home-assistant/qrcode/tokens/TKN?clientid=HA_3y9q4ak7g4ephrvke&usercode=Abc123"
    );
    let f = Fake::new(|_| Ok(login_ok("https://apigw.tuyaeu.com/")));
    assert!(poll_login(&f, &code(), &qr()).unwrap().is_some());
    let f = Fake::new(|_| Ok(r#"{"success":true,"result":{"access_token":"a"}}"#.into()));
    assert_eq!(
        poll_login(&f, &code(), &qr()).err(),
        Some(CloudError::Decode)
    );
}

#[test]
fn rejects_endpoint_outside_allowlist() {
    for ep in [
        "http://apigw.tuyaeu.com",
        "https://evil.example",
        "https://tuyaeu.com.evil.example",
        "https://eviltuyaeu.com",
        "https://apigw.tuyaeu.com@evil.example",
        "https://apigw.tuyaeu.com:8443",
        "https://apigw.tuyaeu.com/path",
        "https://apigw.tuyaeu.com?x=1",
        "https://apigw.tuyaeu.com//",
        "https://.tuyaeu.com",
        "https://",
        "",
    ] {
        let endpoint = ep.to_owned();
        let f = Fake::new(move |_| Ok(login_ok(&endpoint)));
        assert_eq!(
            poll_login(&f, &code(), &qr()).err(),
            Some(CloudError::EndpointNotAllowed),
            "{ep}"
        );
        assert_eq!(f.seen.borrow().len(), 1, "no request after rejection: {ep}");
    }
    for ep in [
        "https://apigw.tuyaeu.com",
        "https://APIGW.TuyaUS.com/",
        "https://tuyain.com",
    ] {
        let endpoint = ep.to_owned();
        let f = Fake::new(move |_| Ok(login_ok(&endpoint)));
        assert!(poll_login(&f, &code(), &qr()).unwrap().is_some(), "{ep}");
    }
}

fn devices_handler(req: &HttpRequest) -> Result<String, CloudError> {
    if req.url.contains("/users/homes") {
        Ok(reply(
            req,
            r#"[{"ownerId":111,"name":"A"},{"ownerId":"222","name":"B"}]"#,
        ))
    } else if req.url.contains("/ha/home/devices") {
        let plain = crypto::decrypt_response(&raw_encdata(req), &secret_of(req)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&plain).unwrap();
        if v["homeId"] == "111" {
            Ok(reply(
                req,
                r#"[{"id":"d1","name":"Strip","category":"dd","product_name":"JSK","online":true,"local_key":"LK1","extra":1}]"#,
            ))
        } else {
            assert_eq!(plain, r#"{"homeId":"222"}"#);
            Ok(reply(
                req,
                r#"[{"id":"d2","name":"Bulb","category":"dj","product_name":"B","online":false,"local_key":"LK2"}]"#,
            ))
        }
    } else {
        Err(CloudError::Network)
    }
}

#[test]
fn lists_devices_across_homes_and_flags_light_strips() {
    let f = Fake::new(devices_handler);
    let devs = list_devices(&f, &session()).unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].id, "d1");
    assert!(devs[0].is_light_strip());
    assert!(devs[0].online);
    assert_eq!(devs[0].local_key.expose_secret(), "LK1");
    assert!(!devs[1].is_light_strip());
    assert!(
        f.urls()
            .iter()
            .all(|u| u.starts_with("https://apigw.tuyaeu.com/"))
    );
}

#[test]
fn device_missing_local_key_is_decode_error() {
    let f = Fake::new(|r| {
        if r.url.contains("/users/homes") {
            Ok(reply(r, r#"[{"ownerId":1}]"#))
        } else {
            Ok(reply(
                r,
                r#"[{"id":"d","name":"n","category":"dd","product_name":"p","online":true}]"#,
            ))
        }
    });
    assert_eq!(list_devices(&f, &session()).err(), Some(CloudError::Decode));
}

#[test]
fn api_error_on_signed_call_carries_code() {
    let f = Fake::new(|_| Ok(r#"{"success":false,"code":"2017","msg":"m"}"#.into()));
    assert_eq!(
        list_devices(&f, &session()).err(),
        Some(CloudError::Api {
            code: "2017".into()
        })
    );
}

#[test]
fn signed_requests_carry_valid_signature() {
    let f = Fake::new(devices_handler);
    list_devices(&f, &session()).unwrap();
    let revoker = Fake::new(|r| Ok(reply(r, "true")));
    revoke(&revoker, session()).unwrap();
    let all: Vec<HttpRequest> = f
        .seen
        .borrow()
        .iter()
        .chain(revoker.seen.borrow().iter())
        .cloned()
        .collect();
    assert_eq!(all.len(), 4);
    for r in &all {
        let rid = header(r, "X-requestId");
        assert_eq!(rid.len(), 36);
        assert_eq!(&rid[14..15], "4");
        let hk = crypto::hash_key(rid, REFRESH);
        let hdrs: Vec<(&str, &str)> = ["X-appKey", "X-requestId", "X-sid", "X-time", "X-token"]
            .into_iter()
            .map(|k| (k, header(r, k)))
            .collect();
        let (q, b) = match r.method {
            Method::Get => (raw_encdata(r), String::new()),
            Method::Post => (String::new(), raw_encdata(r)),
        };
        assert_eq!(header(r, "X-sign"), crypto::sign(&hk, &hdrs, &q, &b));
        assert_eq!(header(r, "X-appKey"), CLIENT_ID);
        assert_eq!(header(r, "X-token"), ACCESS);
    }
    let body: serde_json::Value = serde_json::from_str(
        &crypto::decrypt_response(&raw_encdata(&all[3]), &secret_of(&all[3])).unwrap(),
    )
    .unwrap();
    assert_eq!(body["accessToken"], ACCESS);
    assert_eq!(body["terminalId"], "term1");
    assert!(all[3].url.ends_with("/v1.0/m/token/terminal/expire"));
}

#[test]
fn revoke_runs_when_listing_fails() {
    let f = Fake::new(|r| {
        if r.url.contains("/users/homes") {
            Err(CloudError::Http(500))
        } else {
            Ok(reply(r, "true"))
        }
    });
    let out = run_and_revoke(&f, session(), |t, s| list_devices(t, s));
    assert_eq!(out.err(), Some(CloudError::Http(500)));
    let urls = f.urls();
    assert!(
        urls.last()
            .unwrap()
            .ends_with("/v1.0/m/token/terminal/expire")
    );
    assert_eq!(f.seen.borrow().last().unwrap().method, Method::Post);
}

#[test]
fn debug_output_never_contains_secrets() {
    let s = session();
    let dev = CloudDevice {
        id: "d".into(),
        name: "n".into(),
        category: "dd".into(),
        product_name: "p".into(),
        online: true,
        local_key: "LK-SECRET".to_owned().into(),
    };
    let qr = QrLogin::for_token("TKN-SECRET");
    let uc = UserCode::parse("UC-SECRET".replace('-', "").as_str()).unwrap();
    let out = format!("{s:?} {dev:?} {qr:?} {uc:?}");
    for secret in [
        "at-SECRET",
        "rt-SECRET",
        "LK-SECRET",
        "TKN-SECRET",
        "UCSECRET",
        "u1",
        "term1",
    ] {
        assert!(!out.contains(secret), "{secret} leaked: {out}");
    }
}

#[test]
fn ureq_transport_refuses_plain_http_without_connecting() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", l.local_addr().unwrap());
    let req = HttpRequest {
        method: Method::Get,
        url,
        headers: vec![],
        body: None,
    };
    assert!(UreqTransport::new().send(&req).is_err());
    assert!(l.accept().is_err(), "transport must not connect over http");
}
