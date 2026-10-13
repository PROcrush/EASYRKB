//! Vocal presence (`PVDI`) — **placeholder**.
//!
//! rekordbox 7 marks where vocals are, which the CDJ-3000 draws on the
//! waveform. It needs a trained model; guessing would put vocal markers in the
//! wrong places, so this is unimplemented and the tag is omitted.

/// Fraction of vocal presence per waveform column, 0..=1.
#[derive(Debug, Clone, Default)]
pub struct VocalTrack {
    pub per_column: Vec<f32>,
}

pub trait VocalDetector {
    fn vocals(&self, samples: &[f32], sample_rate: u32) -> Option<VocalTrack>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Unimplemented;

impl VocalDetector for Unimplemented {
    fn vocals(&self, _samples: &[f32], _sample_rate: u32) -> Option<VocalTrack> {
        None
    }
}
