//! Helpers for faking the cloud in tests of other crates (feature `test-support`).

use crate::crypto;
use crate::transport::HttpRequest;

/// Builds the encrypted success answer a real signed endpoint would give for `req`,
/// for a session whose refresh token is `refresh_token`.
///
/// # Panics
/// Test helper: panics if the request has no `X-requestId` header or encryption fails.
#[must_use]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub fn encrypted_reply(req: &HttpRequest, refresh_token: &str, result: &str) -> String {
    let rid = req
        .headers
        .iter()
        .find(|(k, _)| k == "X-requestId")
        .map(|(_, v)| v.as_str())
        .expect("signed request");
    let secret = crypto::secret(rid, &crypto::hash_key(rid, refresh_token));
    let enc = crypto::encrypt_request(result, &secret, b"abcdefghijkl").unwrap();
    serde_json::json!({"success": true, "result": enc, "t": 1}).to_string()
}
