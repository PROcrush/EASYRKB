//! Identifiers in the shapes rekordbox uses.
//!
//! Three shapes, all observed in the reference library:
//!
//! - `djmdContent.ID` — a decimal string, largest seen 268,438,243.
//! - `djmdPlaylist.ID` — a decimal string, largest seen 4,290,236,987.
//! - `djmdCue.ID` — a decimal string too, largest seen 4,294,966,064 [OBS]
//!   across all 1,041,056 rows; not one is a UUID.
//! - `djmdSongPlaylist.ID` and every `UUID` column — a version 4 UUID.
//!
//! A generated id is always checked against the table before it is used, so
//! the generator only has to be well spread, not cryptographically strong.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Distinguishes generators created within the same clock tick.
///
/// The clock alone is not enough: two `from_entropy` calls in immediate
/// succession can read the same nanosecond and, being stack locals of the same
/// depth, the same address — which produced two generators emitting an
/// identical stream of ids. A test caught it.
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Largest `djmdContent.ID` observed in the reference library, rounded down to
/// the power of two below it. New content ids stay under this so they sort and
/// display like rekordbox's own.
pub const MAX_CONTENT_ID: u64 = 1 << 28;

/// `djmdPlaylist.ID` values run to just under 2^32.
pub const MAX_PLAYLIST_ID: u64 = 1 << 32;

/// `djmdCue.ID` values run to just under 2^32 as well, and the index keeps a
/// cue's id as a `u32` on the strength of it.
pub const MAX_CUE_ID: u64 = 1 << 32;

/// xorshift64*, seeded from the clock. Small, fast, and adequate: every id it
/// produces is checked for collision before use.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// Seeds from the clock, a stack address, and a process-wide counter.
    ///
    /// The counter is what guarantees two generators differ: the other two can
    /// both repeat inside a single clock tick.
    #[must_use]
    pub fn from_entropy() -> Self {
        // Built from seconds and the sub-second part rather than `as_nanos`,
        // which is a u128 and would need a truncating cast.
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| {
            d.as_secs().wrapping_mul(1_000_000_000).wrapping_add(u64::from(d.subsec_nanos()))
        });
        let local = 0_u8;
        let address = std::ptr::from_ref(&local) as u64;
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        // Multiplied by an odd constant so successive counter values land far
        // apart in the state space rather than in adjacent slots.
        Self::from_seed(
            nanos ^ address.rotate_left(17) ^ sequence.wrapping_mul(0x9e37_79b9_7f4a_7c15),
        )
    }

    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        // Zero is a fixed point of xorshift, so it must never be the state.
        Self(if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A decimal id below `limit`, never zero.
    pub fn numeric_id(&mut self, limit: u64) -> String {
        let value = self.next_u64() % limit.max(2);
        format!("{}", value.max(1))
    }

    /// A version 4 UUID, lowercase and hyphenated as rekordbox stores them.
    pub fn uuid4(&mut self) -> String {
        let (high, low) = (self.next_u64(), self.next_u64());
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&high.to_be_bytes());
        bytes[8..].copy_from_slice(&low.to_be_bytes());
        // Version 4 in the high nibble of byte 6, variant 10 in byte 8.
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex = |slice: &[u8]| -> String {
            const DIGITS: &[u8; 16] = b"0123456789abcdef";
            let mut out = String::with_capacity(slice.len() * 2);
            for byte in slice {
                out.push(char::from(DIGITS[usize::from(byte >> 4)]));
                out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
            }
            out
        };
        format!(
            "{}-{}-{}-{}-{}",
            hex(&bytes[0..4]),
            hex(&bytes[4..6]),
            hex(&bytes[6..8]),
            hex(&bytes[8..10]),
            hex(&bytes[10..16])
        )
    }
}
