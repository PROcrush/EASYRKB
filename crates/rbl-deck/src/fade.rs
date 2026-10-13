//! The two milliseconds at either end of a sound.
//!
//! Audio does not start and stop at zero of its own accord. Press play and the
//! first sample handed to the device is wherever the waveform happened to be —
//! on a kick that is most of full scale, and a step from silence to full scale
//! is a click. Pause, seek, cue, jump a beat: each one is the same step, taken
//! in the other direction or in both. The fix is the same every time, so it
//! lives here rather than in each of them: a gain that walks to where it is
//! asked for instead of jumping, so that audio is only ever started or stopped
//! where the gain is already zero. Nothing in the deck cuts — not a pause, not
//! a seek, not the end of a file, not even the decode thread failing to keep
//! up, which holds its last frame and fades that instead.

/// Frames a start or a stop is faded over: about two milliseconds at 44.1 kHz.
///
/// Short enough that a stop still reads as a stop — two milliseconds is under
/// a tenth of the shortest thing anybody hears as a note — and long enough to
/// have no edge in it. Measured in frames rather than seconds because the
/// callback counts frames, and a device at 48 kHz fading over the same count
/// is 1.8 ms, which is the same thing.
pub const FADE_FRAMES: u16 = 88;

/// A gain that moves towards what it is asked for, one step a frame.
///
/// Not a filter and not a curve: a straight line, because it is short enough
/// that the shape of it cannot be heard and a line is exact about reaching
/// zero. Reaching zero is the whole point — audio is only ever started or cut
/// where this reads zero.
///
/// Counted in frames rather than accumulated as a float. Eighty-eight
/// additions of one eighty-eighth land six ten-millionths short of zero, and
/// six ten-millionths of a kick drum multiplied back in at the end of a fade
/// is the click, quieter. A count divides exactly.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ramp {
    /// Frames into the fade, from 0 (silent) to `FADE_FRAMES` (open).
    at: u16,
}

impl Ramp {
    /// A ramp at zero, which is where everything that is not sounding sits.
    pub const fn silent() -> Self {
        Self { at: 0 }
    }

    /// Exactly 0 while silent and exactly 1 while open, by construction.
    pub fn gain(self) -> f32 {
        f32::from(self.at) / f32::from(FADE_FRAMES)
    }

    /// Whether the sound has gone: nothing is being heard through this.
    pub fn silent_now(self) -> bool {
        self.at == 0
    }

    /// Whether the sound is all the way up, so nothing is being faded.
    #[cfg_attr(not(test), allow(dead_code, reason = "read by the fade tests"))]
    pub fn open(self) -> bool {
        self.at >= FADE_FRAMES
    }

    /// Moves one frame, up if `open` and down if not, and returns the gain to
    /// multiply that frame by.
    pub fn step(&mut self, open: bool) -> f32 {
        if open {
            self.at = self.at.saturating_add(1).min(FADE_FRAMES);
        } else {
            self.at = self.at.saturating_sub(1);
        }
        self.gain()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "the ramp reaches exactly zero and one, and that is the point")]
mod tests {
    use super::*;

    /// The gain a frame is multiplied by, `frames` into a fade in.
    fn after(frames: usize, open: bool) -> Ramp {
        let mut ramp = Ramp::silent();
        for _ in 0..frames {
            ramp.step(open);
        }
        ramp
    }

    #[test]
    fn a_fade_in_starts_at_zero_and_reaches_exactly_full() {
        let mut ramp = Ramp::silent();
        assert_eq!(ramp.gain(), 0.0);
        assert_eq!(ramp.step(true), 1.0 / f32::from(FADE_FRAMES));
        let open = after(usize::from(FADE_FRAMES), true);
        assert!(open.open());
        assert_eq!(open.gain(), 1.0);
    }

    #[test]
    fn a_fade_out_ends_at_exactly_zero() {
        // Not "close to zero": the last sample of a stop is multiplied by this,
        // and anything left over is the click, quieter.
        let mut ramp = after(200, true);
        for _ in 0..FADE_FRAMES {
            ramp.step(false);
        }
        assert_eq!(ramp.gain(), 0.0);
        assert!(ramp.silent_now());
    }

    #[test]
    fn it_takes_exactly_two_milliseconds_and_a_step_at_a_time() {
        let mut ramp = Ramp::silent();
        let mut last = 0.0;
        let mut frames = 0_u16;
        while !ramp.open() {
            let gain = ramp.step(true);
            assert!(gain - last <= 1.0 / f32::from(FADE_FRAMES) + 1e-6, "stepped by {}", gain - last);
            last = gain;
            frames += 1;
            assert!(frames <= FADE_FRAMES, "still fading after {frames} frames");
        }
        assert_eq!(frames, FADE_FRAMES);
    }

    #[test]
    fn a_ramp_held_down_stays_at_zero_rather_than_going_under() {
        let mut ramp = Ramp::silent();
        for _ in 0..1_000 {
            assert_eq!(ramp.step(false), 0.0);
        }
    }
}
