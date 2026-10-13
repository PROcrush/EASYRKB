//! Phrase detection (`PSSI`) — **placeholder**.
//!
//! rekordbox labels intro / verse / chorus / up / down / outro. No open
//! implementation exists, and the labels a wrong detector produces would be
//! visibly wrong to a DJ mid-set, so this is deliberately unimplemented rather
//! than approximated. The trait fixes the shape so the ANLZ writer can omit the
//! tag cleanly until a real detector exists.

/// One labelled section of a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    pub start_beat: u32,
    pub kind: PhraseKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseKind {
    Intro,
    Verse,
    Bridge,
    Chorus,
    Up,
    Down,
    Outro,
}

pub trait PhraseAnalyzer {
    /// Returns `None` when phrases cannot be determined.
    fn phrases(&self, samples: &[f32], sample_rate: u32) -> Option<Vec<Phrase>>;
}

/// The stub in use. Always returns `None`, so the ANLZ writer omits `PSSI`
/// rather than writing invented structure.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unimplemented;

impl PhraseAnalyzer for Unimplemented {
    fn phrases(&self, _samples: &[f32], _sample_rate: u32) -> Option<Vec<Phrase>> {
        None
    }
}
