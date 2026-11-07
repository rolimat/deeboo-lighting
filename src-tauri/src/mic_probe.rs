//! Debug probe: opens the default input for 1 s and reports the peak level, to prove the
//! microphone entitlement and prompt work in the App Sandbox.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const FLOOR_DBFS: f32 = -120.0;

fn peak_to_dbfs(peak: f32) -> f32 {
    if peak <= 1e-6 {
        FLOOR_DBFS
    } else {
        20.0 * peak.log10()
    }
}

/// Peak of the default input over one second, in dBFS.
///
/// # Errors
/// Returns a message if there is no input, the format is unsupported or the stream fails.
pub fn peak_dbfs() -> Result<f32, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("no input device")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    // shortcut: only f32 input (what CoreAudio gives); handle i16 if another host ever needs it.
    if config.sample_format() != SampleFormat::F32 {
        return Err("unsupported sample format".into());
    }
    // Non-negative f32 bits sort like the floats, so `fetch_max` tracks the peak.
    let peak = Arc::new(AtomicU32::new(0));
    let sink = Arc::clone(&peak);
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                for s in data {
                    sink.fetch_max(s.abs().to_bits(), Ordering::Relaxed);
                }
            },
            |_| {},
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_secs(1));
    drop(stream);
    let bits = peak.load(Ordering::Relaxed);
    let v = f32::from_bits(bits);
    Ok(peak_to_dbfs(if v.is_nan() { 0.0 } else { v }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbfs_of_full_scale_half_and_silence() {
        assert!((peak_to_dbfs(1.0) - 0.0).abs() < 1e-4);
        assert!((peak_to_dbfs(0.5) + 6.0206).abs() < 1e-3);
        assert!((peak_to_dbfs(0.0) + 120.0).abs() < 1e-4);
    }
}
