//! `PVDI` — where rekordbox heard a voice.
//!
//! Not in any published spec; what follows was measured on the reference
//! library rather than read anywhere.
//!
//! **Framing** [OBS, 600 of 600 `.2EX` files]. `len_header` is 24, so the tag
//! header is three big-endian words:
//!
//! ```text
//! 0c  0x0000_0400   1024, constant
//! 10  0x5622_0001   constant
//! 14  entry count   equal to the payload length in every file
//! ```
//!
//! One unsigned byte per entry, so the third word is both the count and the
//! byte length. Note this is *not* the `len_entry_bytes, len_entries, flags`
//! shape the waveform tags use: the first word is 1024, never 1.
//!
//! **Resolution** [OBS]. Dividing the `PWV7` column count by the entry count
//! gives 6.961 to 6.967 across 600 tracks (median 6.966). `PWV7` runs at 150
//! columns a second, which puts `PVDI` at 21.534 entries a second, or 46.44 ms
//! each. Checked the other way, against the last beat of each track's `PQTZ`
//! grid, the same files give 46.28 to 46.43 ms an entry. That rate is
//! 44100/2048, and equally 22050/1024 — which is what the constant 1024 in the
//! first header word looks like: a hop over a half-rate signal. Which of the
//! two it is is [UNKNOWN] and does not matter to a caller; the measured rate
//! is what [`VOCAL_FRAME_MS`] states.
//!
//! **Values** [OBS]. 4.6 million bytes across those 600 files take only the
//! values 0, 1, 2, 3 and 4. Runs of 4 are bounded by short ramps through 3, 2
//! and 1, so it reads as an intensity rather than a flag.
//!
//! **That it is vocals** [OBS]. 19 tracks whose title says a cappella are 87%
//! to 99% non-zero; 16 whose title says instrumental or dub are 0% to 8%. That
//! separation is what the name is based on.

use crate::Section;
use rbl_core::FourCc;

/// Milliseconds one entry covers, measured (see the module docs).
pub const VOCAL_FRAME_MS: f64 = 1000.0 * 2048.0 / 44_100.0;

/// The largest value seen in the reference library. Callers scaling to a
/// display height should divide by this.
pub const VOCAL_MAX: u8 = 4;

impl Section {
    /// `PVDI` — one intensity byte per 46.44 ms frame, 0 to [`VOCAL_MAX`].
    ///
    /// `None` for any other tag, and for a `PVDI` whose declared count does
    /// not match its payload, which would mean the layout above is not what
    /// this file holds.
    #[must_use]
    pub fn as_vocals(&self) -> Option<&[u8]> {
        if self.tag != FourCc::new(b"PVDI") {
            return None;
        }
        let declared = u32::from_be_bytes([
            self.header.get(8).copied()?,
            self.header.get(9).copied()?,
            self.header.get(10).copied()?,
            self.header.get(11).copied()?,
        ]) as usize;
        (declared == self.payload.len()).then_some(self.payload.as_slice())
    }
}

impl crate::Anlz {
    /// The vocal-presence strip, if the file carries one.
    ///
    /// Borrowed rather than copied: a long track has tens of thousands of
    /// entries and a caller normally wants a window of them.
    #[must_use]
    pub fn vocals(&self) -> Option<&[u8]> {
        self.sections.iter().find_map(Section::as_vocals)
    }
}
