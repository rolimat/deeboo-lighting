//! HTTP transport seam. `UreqTransport` is the only real network code in this crate.

use crate::CloudError;
use std::time::Duration;

const BODY_LIMIT: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// Headers and body can carry tokens, so `Debug` is hand-written and shows only method and host (paths and queries carry tokens).
#[derive(Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let host = self
            .url
            .strip_prefix("https://")
            .and_then(|r| r.split(['/', '?']).next())
            .unwrap_or("[non-https]");
        write!(f, "HttpRequest({:?} {host})", self.method)
    }
}

/// Returns the response body of a 2xx answer; anything else is an error.
pub trait Transport {
    /// # Errors
    /// `Http(status)` for non-2xx, `Network` for connection, TLS, timeout or oversize failures.
    fn send(&self, req: &HttpRequest) -> Result<String, CloudError>;
}

/// HTTPS only, 10 s total timeout, no redirects, 1 MiB response limit, the OS trust store (platform verifier).
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    #[must_use]
    pub fn new() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(true)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .unversioned_rustls_crypto_provider(std::sync::Arc::new(
                        rustls::crypto::ring::default_provider(),
                    ))
                    .build(),
            )
            .timeout_global(Some(TIMEOUT))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for UreqTransport {
    fn send(&self, req: &HttpRequest) -> Result<String, CloudError> {
        let result = match req.method {
            Method::Get => {
                let mut b = self.agent.get(&req.url);
                for (k, v) in &req.headers {
                    b = b.header(k, v);
                }
                b.call()
            }
            Method::Post => {
                let mut b = self.agent.post(&req.url);
                for (k, v) in &req.headers {
                    b = b.header(k, v);
                }
                b.send(req.body.as_deref().unwrap_or_default())
            }
        };
        let mut resp = result.map_err(|_| CloudError::Network)?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CloudError::Http(status));
        }
        resp.body_mut()
            .with_config()
            .limit(BODY_LIMIT)
            .read_to_string()
            .map_err(|_| CloudError::Network)
    }
}
