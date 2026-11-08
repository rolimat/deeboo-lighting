//! MP3/WAV decoding to mono `f32` chunks (stereo is averaged).

use std::fmt;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug)]
pub enum DecodeError {
    Io(std::io::Error),
    Codec(String),
    NoAudioTrack,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Io(e) => write!(f, "cannot read audio file: {e}"),
            DecodeError::Codec(e) => write!(f, "cannot decode audio: {e}"),
            DecodeError::NoAudioTrack => write!(f, "no audio track found"),
        }
    }
}

impl From<SymError> for DecodeError {
    fn from(e: SymError) -> Self {
        match e {
            SymError::IoError(e) => DecodeError::Io(e),
            other => DecodeError::Codec(other.to_string()),
        }
    }
}

pub struct MonoChunks {
    pub sample_rate: u32,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    done: bool,
}

pub fn mono_chunks(path: &Path) -> Result<MonoChunks, DecodeError> {
    let file = File::open(path).map_err(DecodeError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let format = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    Ok(MonoChunks {
        sample_rate,
        format,
        decoder,
        track_id,
        done: false,
    })
}

impl MonoChunks {
    fn next_chunk(&mut self) -> Result<Option<Vec<f32>>, DecodeError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    let spec = *buf.spec();
                    let channels = spec.channels.count().max(1);
                    let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                    sb.copy_interleaved_ref(buf);
                    #[allow(clippy::cast_precision_loss)]
                    let mono = sb
                        .samples()
                        .chunks_exact(channels)
                        .map(|fr| fr.iter().sum::<f32>() / channels as f32)
                        .collect();
                    return Ok(Some(mono));
                }
                // A corrupt frame is skipped, like a player would.
                Err(SymError::DecodeError(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Iterator for MonoChunks {
    type Item = Result<Vec<f32>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.next_chunk() {
            Ok(Some(c)) => Some(Ok(c)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16-bit mono PCM WAV.
    pub fn wav_bytes(rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_len = u32::try_from(samples.len() * 2).unwrap();
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }

    #[test]
    fn decode_reads_a_generated_wav() {
        let rate = 44_100u32;
        let samples: Vec<i16> = (0..rate)
            .map(|i| {
                let t = f64::from(i) / f64::from(rate);
                #[allow(clippy::cast_possible_truncation)]
                let s = ((2.0 * std::f64::consts::PI * 440.0 * t).sin() * 16_000.0) as i16;
                s
            })
            .collect();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        std::fs::write(&path, wav_bytes(rate, &samples)).unwrap();
        let chunks = mono_chunks(&path).unwrap();
        assert_eq!(chunks.sample_rate, rate);
        let all: Vec<f32> = chunks.flat_map(|c| c.unwrap()).collect();
        assert_eq!(all.len(), rate as usize);
        assert!(all.iter().any(|s| s.abs() > 0.4));
    }

    #[test]
    fn missing_file_is_a_typed_error() {
        assert!(matches!(
            mono_chunks(Path::new("/nonexistent/x.mp3")),
            Err(DecodeError::Io(_))
        ));
    }
}
