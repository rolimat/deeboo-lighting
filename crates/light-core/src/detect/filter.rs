//! Biquad IIR filters (RBJ Audio EQ Cookbook) and the detection bands.

/// Transposed direct form II biquad.
#[derive(Clone, Debug)]
pub struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl Biquad {
    fn new(b: [f32; 3], a0: f32, a1: f32, a2: f32) -> Biquad {
        Biquad {
            b: [b[0] / a0, b[1] / a0, b[2] / a0],
            a: [a1 / a0, a2 / a0],
            z: [0.0; 2],
        }
    }

    #[must_use]
    pub fn lowpass(fc: f32, q: f32, fs: f32) -> Biquad {
        let (cos, alpha) = Self::coeffs(fc, q, fs);
        let b1 = 1.0 - cos;
        Biquad::new(
            [b1 / 2.0, b1, b1 / 2.0],
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    #[must_use]
    pub fn highpass(fc: f32, q: f32, fs: f32) -> Biquad {
        let (cos, alpha) = Self::coeffs(fc, q, fs);
        let b1 = 1.0 + cos;
        Biquad::new(
            [b1 / 2.0, -b1, b1 / 2.0],
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    fn coeffs(fc: f32, q: f32, fs: f32) -> (f32, f32) {
        let w0 = 2.0 * std::f32::consts::PI * fc / fs;
        (w0.cos(), w0.sin() / (2.0 * q))
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// Biquads in series.
#[derive(Clone, Debug)]
pub struct Cascade<const N: usize>([Biquad; N]);

impl<const N: usize> Cascade<N> {
    pub fn process(&mut self, x: f32) -> f32 {
        self.0.iter_mut().fold(x, |s, f| f.process(s))
    }
}

/// Butterworth Q values of the two sections of a 4th-order filter.
const Q4: [f32; 2] = [0.5412, 1.3066];

#[must_use]
pub fn kick_band(fs: f32) -> Cascade<4> {
    Cascade([
        Biquad::highpass(40.0, Q4[0], fs),
        Biquad::highpass(40.0, Q4[1], fs),
        Biquad::lowpass(120.0, Q4[0], fs),
        Biquad::lowpass(120.0, Q4[1], fs),
    ])
}

#[must_use]
pub fn sub_band(fs: f32) -> Cascade<2> {
    Cascade([
        Biquad::lowpass(60.0, Q4[0], fs),
        Biquad::lowpass(60.0, Q4[1], fs),
    ])
}

#[must_use]
pub fn high_band(fs: f32) -> Cascade<2> {
    Cascade([
        Biquad::highpass(4000.0, Q4[0], fs),
        Biquad::highpass(4000.0, Q4[1], fs),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Steady-state RMS gain in dB of a unit sine over the last 0.5 s of 1 s.
    // Test helper: sample counts and dB values are small, exact in f32/f64.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn gain_db<const N: usize>(mut f: Cascade<N>, freq: f32, fs: f32) -> f32 {
        let n = fs as usize;
        let (mut acc, mut cnt) = (0.0_f64, 0_u32);
        for i in 0..n {
            let x = (2.0 * std::f32::consts::PI * freq * i as f32 / fs).sin();
            let y = f.process(x);
            if i >= n / 2 {
                acc += f64::from(y * y);
                cnt += 1;
            }
        }
        let rms_out = (acc / f64::from(cnt)).sqrt();
        (20.0 * (rms_out / std::f64::consts::FRAC_1_SQRT_2).log10()) as f32
    }

    fn check_kick(fs: f32) {
        let g80 = gain_db(kick_band(fs), 80.0, fs);
        let g1k = gain_db(kick_band(fs), 1000.0, fs);
        let g20 = gain_db(kick_band(fs), 20.0, fs);
        println!("kick fs={fs}: 80Hz={g80:.2} 1k={g1k:.2} 20Hz={g20:.2}");
        assert!(g80 >= -3.0, "80 Hz {g80}");
        assert!(g1k <= -40.0, "1 kHz {g1k}");
        assert!(g20 <= -12.0, "20 Hz {g20}");
    }

    #[test]
    fn kick_band_passes_80hz_and_rejects_1khz() {
        check_kick(48_000.0);
    }

    #[test]
    fn sub_band_passes_40hz_rejects_200hz() {
        let g40 = gain_db(sub_band(48_000.0), 40.0, 48_000.0);
        let g200 = gain_db(sub_band(48_000.0), 200.0, 48_000.0);
        println!("sub: 40Hz={g40:.2} 200Hz={g200:.2}");
        assert!(g40 >= -3.0, "40 Hz {g40}");
        assert!(g200 <= -30.0, "200 Hz {g200}");
    }

    #[test]
    fn high_band_passes_8khz_rejects_500hz() {
        let g8k = gain_db(high_band(48_000.0), 8000.0, 48_000.0);
        let g500 = gain_db(high_band(48_000.0), 500.0, 48_000.0);
        println!("high: 8k={g8k:.2} 500Hz={g500:.2}");
        assert!(g8k >= -3.0, "8 kHz {g8k}");
        assert!(g500 <= -40.0, "500 Hz {g500}");
    }

    #[test]
    fn filters_hold_at_common_sample_rates() {
        for fs in [44_100.0, 48_000.0, 96_000.0] {
            check_kick(fs);
        }
    }
}
