//! QR code matrix for the connection wizard. The payload is a secret (QR token), so the
//! matrix is the only thing that crosses the IPC boundary and `Debug` never prints modules.

use qrcode::{Color, QrCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use std::fmt;

/// Row-major dark/light modules of a square QR code.
#[derive(Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QrMatrix {
    pub size: u16,
    pub modules: Vec<bool>,
}

impl fmt::Debug for QrMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QrMatrix({}x{})", self.size, self.size)
    }
}

/// Encodes `payload` as a QR code.
///
/// # Errors
/// Returns a fixed message (never the payload) if it cannot be encoded.
pub fn qr_matrix(payload: &SecretString) -> Result<QrMatrix, String> {
    let code = QrCode::new(payload.expose_secret().as_bytes())
        .map_err(|_| "cannot build the QR code".to_owned())?;
    let size = u16::try_from(code.width()).map_err(|_| "QR code too large".to_owned())?;
    let modules = code
        .to_colors()
        .into_iter()
        .map(|c| c == Color::Dark)
        .collect();
    Ok(QrMatrix { size, modules })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = "https://example.invalid/qr?token=SECRETTOKEN123";

    #[test]
    fn qr_matrix_is_square_and_decodable() {
        let m = qr_matrix(&SecretString::from(PAYLOAD)).unwrap();
        assert_eq!(m.modules.len(), usize::from(m.size) * usize::from(m.size));
        let want: Vec<bool> = QrCode::new(PAYLOAD)
            .unwrap()
            .to_colors()
            .into_iter()
            .map(|c| c == Color::Dark)
            .collect();
        assert_eq!(m.modules, want);
    }

    #[test]
    fn qr_debug_hides_modules() {
        let m = qr_matrix(&SecretString::from(PAYLOAD)).unwrap();
        let s = format!("{m:?}");
        assert_eq!(s, format!("QrMatrix({0}x{0})", m.size));
        assert!(!s.contains("true") && !s.contains("false"));
    }

    #[test]
    fn qr_error_does_not_leak_payload() {
        let big = "SECRETTOKEN".repeat(500);
        let e = qr_matrix(&SecretString::from(big)).unwrap_err();
        assert!(!e.contains("SECRET"));
    }
}
