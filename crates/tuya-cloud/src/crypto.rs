//! Port of the signing/crypto helpers of `tuya-device-sharing-sdk` 0.2.15 (MIT), `customerapi.py`.

use crate::CloudError;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha2::Sha256;
use std::fmt::Write as _;
use zeroize::Zeroizing;

const ALPHABET: &[u8; 48] = b"ABCDEFGHJKMNPQRSTWXYZabcdefhijkmnprstwxyz2345678";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn hmac_hex(key: &[u8], msg: &[u8]) -> String {
    // HMAC accepts any key length, so new_from_slice cannot fail; empty output is only a fallback.
    let Ok(mut mac) = <Hmac<Sha256> as Mac>::new_from_slice(key) else {
        return String::new();
    };
    mac.update(msg);
    hex(&mac.finalize().into_bytes())
}

pub(crate) fn hash_key(request_id: &str, refresh_token: &str) -> String {
    let mut h = Md5::new();
    h.update(request_id.as_bytes());
    h.update(refresh_token.as_bytes());
    hex(&h.finalize())
}

pub(crate) fn secret(request_id: &str, hash_key: &str) -> String {
    let mut s = hmac_hex(request_id.as_bytes(), hash_key.as_bytes());
    s.truncate(16);
    s
}

pub(crate) fn encrypt_request(
    plain: &str,
    secret: &str,
    nonce: &[u8; 12],
) -> Result<String, CloudError> {
    let cipher = Aes128Gcm::new_from_slice(secret.as_bytes()).map_err(|_| CloudError::Crypto)?;
    let ct = cipher
        .encrypt(Nonce::from_slice(nonce), plain.as_bytes())
        .map_err(|_| CloudError::Crypto)?;
    Ok(format!("{}{}", STANDARD.encode(nonce), STANDARD.encode(ct)))
}

pub(crate) fn decrypt_response(b64: &str, secret: &str) -> Result<Zeroizing<String>, CloudError> {
    let raw = STANDARD.decode(b64).map_err(|_| CloudError::Crypto)?;
    if raw.len() < 12 + 16 {
        return Err(CloudError::Crypto);
    }
    let (nonce, ct) = raw.split_at(12);
    let cipher = Aes128Gcm::new_from_slice(secret.as_bytes()).map_err(|_| CloudError::Crypto)?;
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| CloudError::Crypto)?;
    // Wrap the bytes first so they are wiped even when UTF-8 validation fails.
    let plain = Zeroizing::new(plain);
    let text = std::str::from_utf8(&plain).map_err(|_| CloudError::Crypto)?;
    Ok(Zeroizing::new(text.to_owned()))
}

pub(crate) fn sign(
    hash_key: &str,
    headers: &[(&str, &str)],
    query_enc: &str,
    body_enc: &str,
) -> String {
    let mut msg = headers
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("||");
    msg.push_str(query_enc);
    msg.push_str(body_enc);
    hmac_hex(hash_key.as_bytes(), msg.as_bytes())
}

pub(crate) fn random_nonce() -> Result<[u8; 12], CloudError> {
    let mut out = [0u8; 12];
    let mut filled = 0;
    while filled < out.len() {
        let mut buf = [0u8; 16];
        getrandom::fill(&mut buf).map_err(|_| CloudError::Crypto)?;
        // Rejection sampling: 240 = 5 * 48, so bytes below it map to the alphabet without bias.
        for b in buf.into_iter().filter(|b| *b < 240) {
            if filled < out.len() {
                out[filled] = ALPHABET[usize::from(b) % ALPHABET.len()];
                filled += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RID: &str = "00000000-0000-4000-8000-000000000000";
    #[test]
    fn hash_key_matches_sdk() {
        assert_eq!(hash_key(RID, "rt-test"), "5df80a6fadf4fba70391f10983efeda0");
    }
    #[test]
    fn secret_matches_sdk() {
        assert_eq!(
            secret(RID, "5df80a6fadf4fba70391f10983efeda0"),
            "6c9a8ee911bde0c5"
        );
    }
    #[test]
    fn encrypt_matches_sdk() {
        assert_eq!(
            encrypt_request(r#"{"homeId":"123"}"#, "6c9a8ee911bde0c5", b"abcdefghijkl").unwrap(),
            "YWJjZGVmZ2hpamtsdPro72tdHNxiIAvW1NwrNH1ZjyYUJJfER/wjAIlyOyQ="
        );
    }
    #[test]
    fn sign_matches_sdk() {
        assert_eq!(
            sign(
                "5df80a6fadf4fba70391f10983efeda0",
                &[
                    ("X-appKey", "HA_3y9q4ak7g4ephrvke"),
                    ("X-requestId", RID),
                    ("X-sid", ""),
                    ("X-time", "1793990000000"),
                    ("X-token", "at-test")
                ],
                "YWJjZGVmZ2hpamtsdPro72tdHNxiIAvW1NwrNH1ZjyYUJJfER/wjAIlyOyQ=",
                ""
            ),
            "c78c67d36903933a6942cbfacb00e2435693769097236c323a075187ce42c76a"
        );
    }
    #[test]
    fn decrypt_matches_sdk() {
        assert_eq!(
            decrypt_response(
                "YWJjZGVmZ2hpamtsdPrv6yQCIco1f1Q4RtDuL/9NgFVbz0gHtrTb",
                "6c9a8ee911bde0c5"
            )
            .unwrap()
            .as_str(),
            r#"{"ok":true}"#
        );
    }
    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        assert!(
            decrypt_response(
                "YWJjZGVmZ2hpamtsdPrv6yQCIco1f1Q4RtDuL/9NgFVbz0gHtrTc",
                "6c9a8ee911bde0c5"
            )
            .is_err()
        );
        assert!(decrypt_response("YWJj", "6c9a8ee911bde0c5").is_err());
    }
    #[test]
    fn decrypt_failures_are_all_crypto_errors() {
        assert_eq!(
            decrypt_response("!!not base64!!", "6c9a8ee911bde0c5"),
            Err(CloudError::Crypto)
        );
        assert_eq!(
            decrypt_response("", "6c9a8ee911bde0c5"),
            Err(CloudError::Crypto)
        );
    }
    #[test]
    fn encrypt_rejects_wrong_size_secret() {
        assert_eq!(
            encrypt_request("x", "short", b"abcdefghijkl"),
            Err(CloudError::Crypto)
        );
    }
    #[test]
    fn nonce_uses_sdk_alphabet() {
        let n = random_nonce().unwrap();
        assert!(n.iter().all(|c| ALPHABET.contains(c)));
    }
}
