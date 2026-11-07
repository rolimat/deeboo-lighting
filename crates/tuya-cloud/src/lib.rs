//! Smart Life cloud client pieces (QR login, device list). Transport-agnostic: callers pass a `Transport`.

mod crypto;
mod login;
mod transport;

pub use login::*;
pub use transport::*;

#[cfg(test)]
mod login_tests;

/// Errors of the cloud flow. Messages are fixed: they never carry input data, keys or tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudError {
    InvalidUserCode,
    Network,
    Http(u16),
    Api { code: String },
    Decode,
    Crypto,
    EndpointNotAllowed,
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUserCode => f.write_str("invalid user code"),
            Self::Network => f.write_str("network error"),
            Self::Http(status) => write!(f, "unexpected HTTP status {status}"),
            Self::Api { code } => write!(f, "cloud API error {code}"),
            Self::Decode => f.write_str("could not decode cloud response"),
            Self::Crypto => f.write_str("cloud crypto failure"),
            Self::EndpointNotAllowed => f.write_str("endpoint not allowed"),
        }
    }
}

impl std::error::Error for CloudError {}
