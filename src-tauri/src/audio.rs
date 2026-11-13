//! Live audio capture. The cpal callback only downmixes to mono and pushes into a lock-free ring:
//! no lock, no allocation, no log (spec §13). The engine drains the ring once per beat.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::runtime::{AudioInfo, AudioPort};

/// The 4 kHz high-pass of the detector needs at least this rate.
const MIN_RATE: u32 = 16_000;
/// Seconds of audio the ring (and the per-beat buffer) can hold.
pub const RING_SECONDS: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    NoDevice,
    Denied,
    Unsupported,
    Stream,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoDevice => "no audio input device found",
            Self::Denied => "microphone access denied",
            Self::Unsupported => "audio input must be 32-bit float at 16 kHz or more",
            Self::Stream => "the audio stream could not start",
        })
    }
}

/// Rejects rates the detector cannot analyse.
fn check_rate(rate: u32) -> Result<u32, AudioError> {
    if rate < MIN_RATE {
        Err(AudioError::Unsupported)
    } else {
        Ok(rate)
    }
}

/// Averages each interleaved frame to one sample and pushes it; stops when the ring is full.
/// Returns how many samples were pushed. Real-time safe.
#[allow(clippy::cast_precision_loss)] // channel counts are tiny
pub fn downmix_into(interleaved: &[f32], channels: usize, out: &mut Producer<f32>) -> usize {
    if channels == 0 {
        return 0;
    }
    let mut pushed = 0;
    for frame in interleaved.chunks_exact(channels) {
        let mono = frame.iter().sum::<f32>() / channels as f32;
        if out.push(mono).is_err() {
            break;
        }
        pushed += 1;
    }
    pushed
}

/// An open input stream; dropping it closes the device.
pub struct Capture {
    _stream: Stream,
    pub name: String,
    overflow: Arc<AtomicU64>,
    /// Set by the stream's error callback (an atomic store: nothing else runs on the audio thread).
    dead: Arc<AtomicBool>,
}

impl Capture {
    /// Opens `device` (or the default input) and starts feeding the returned ring.
    ///
    /// # Errors
    /// See [`AudioError`].
    pub fn open(device: Option<&str>) -> Result<(Capture, Consumer<f32>, u32), AudioError> {
        let host = cpal::default_host();
        let dev = match device {
            Some(n) => host
                .input_devices()
                .map_err(|_| AudioError::NoDevice)?
                .find(|d| d.name().ok().as_deref() == Some(n)),
            None => host.default_input_device(),
        }
        .ok_or(AudioError::NoDevice)?;
        let cfg = dev
            .default_input_config()
            .map_err(|_| AudioError::NoDevice)?;
        if cfg.sample_format() != SampleFormat::F32 {
            return Err(AudioError::Unsupported);
        }
        let rate = check_rate(cfg.sample_rate().0)?;
        let channels = usize::from(cfg.channels());
        let (mut prod, cons) = RingBuffer::<f32>::new((rate * RING_SECONDS) as usize);
        let overflow = Arc::new(AtomicU64::new(0));
        let lost = Arc::clone(&overflow);
        let dead = Arc::new(AtomicBool::new(false));
        let died = Arc::clone(&dead);
        let stream = dev
            .build_input_stream(
                &cfg.into(),
                move |data: &[f32], _| {
                    let frames = data.len() / channels.max(1);
                    let n = downmix_into(data, channels, &mut prod);
                    lost.fetch_add((frames - n) as u64, Ordering::Relaxed);
                },
                move |_| died.store(true, Ordering::Relaxed), // no logging on the audio thread
                None,
            )
            .map_err(|_| AudioError::Stream)?;
        stream.play().map_err(|_| AudioError::Denied)?;
        let name = dev.name().unwrap_or_else(|_| "Audio input".into());
        Ok((
            Capture {
                _stream: stream,
                name,
                overflow,
                dead,
            },
            cons,
            rate,
        ))
    }

    #[must_use]
    pub fn overflow(&self) -> u64 {
        self.overflow.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn errored(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }
}

/// The real [`AudioPort`]: at most one open capture.
#[derive(Default)]
pub struct AudioIn {
    open: Option<(Capture, Consumer<f32>)>,
}

impl AudioPort for AudioIn {
    fn open(&mut self, device: Option<&str>) -> Result<AudioInfo, AudioError> {
        let (cap, cons, rate) = Capture::open(device)?;
        let name = cap.name.clone();
        self.open = Some((cap, cons));
        Ok(AudioInfo { rate, name })
    }

    fn close(&mut self) {
        self.open = None;
    }

    fn drain(&mut self, buf: &mut Vec<f32>) {
        let Some((_, cons)) = &mut self.open else {
            return;
        };
        while buf.len() < buf.capacity() {
            let Ok(s) = cons.pop() else { break };
            buf.push(s);
        }
    }

    fn overflow(&self) -> u64 {
        self.open.as_ref().map_or(0, |(c, _)| c.overflow())
    }

    fn errored(&self) -> bool {
        self.open.as_ref().is_some_and(|(c, _)| c.errored())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_no_alloc::{AllocDisabler, assert_no_alloc};

    #[global_allocator]
    static A: AllocDisabler = AllocDisabler;

    #[test]
    fn downmix_averages_channels() {
        let (mut p, mut c) = RingBuffer::new(8);
        assert_eq!(downmix_into(&[1.0, 0.0, 0.5, 0.5, -1.0, 1.0], 2, &mut p), 3);
        assert_eq!([c.pop(), c.pop(), c.pop()], [Ok(0.5), Ok(0.5), Ok(0.0)]);
    }

    #[test]
    fn downmix_does_not_allocate() {
        let (mut p, _c) = RingBuffer::new(64);
        let data = [0.25f32; 32];
        let n = assert_no_alloc(|| downmix_into(&data, 2, &mut p));
        assert_eq!(n, 16);
    }

    #[test]
    fn downmix_counts_overflow() {
        let (mut p, _c) = RingBuffer::new(8);
        assert_eq!(downmix_into(&[0.1; 20], 1, &mut p), 8);
    }

    #[test]
    fn rejects_rates_under_16_khz() {
        assert_eq!(check_rate(8_000), Err(AudioError::Unsupported));
        assert_eq!(check_rate(15_999), Err(AudioError::Unsupported));
        assert_eq!(check_rate(16_000), Ok(16_000));
        assert_eq!(check_rate(48_000), Ok(48_000));
    }
}
