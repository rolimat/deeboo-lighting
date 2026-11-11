//! Audio moment detection building blocks (IIR band filters, no FFT).
pub mod bands;
pub mod filter;
pub mod moment;
pub use bands::BandAnalyzer;
pub use moment::{BeatFeatures, DetectorParams, Moment, MomentDetector, PositionRules};

#[cfg(test)]
mod tests {
    #[test]
    fn detector_types_are_reexported() {
        use crate::detect::{BandAnalyzer, DetectorParams, Moment, MomentDetector};
        let a = BandAnalyzer::new(48_000);
        let d = MomentDetector::new(DetectorParams::default(), 0.0058);
        assert!(a.hop_len() > 0 && d.moment() == Moment::Rhythm);
    }
}
