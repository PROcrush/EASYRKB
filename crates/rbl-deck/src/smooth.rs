//! A gain that walks to where it is asked for.
//!
//! A fader is read once a callback and applied to every frame of it, so moving
//! one is a staircase: the level jumps at each buffer boundary and the size of
//! the step is however far the hand moved in eleven milliseconds. On anything
//! loud that is a rustle, and on a fast move it is a click — the same
//! discontinuity `fade` exists to prevent, arriving from the other side.
//!
//! `fade` is a straight line over two milliseconds, because it has to reach
//! exactly zero at a known moment. This is the other shape: an exponential
//! approach with no end, which is what a knob wants — it starts moving at once
//! and never overshoots, whatever the hand does next.

/// The time constant, in seconds.
///
/// Ten milliseconds is the plan's figure and it is the right order: long
/// enough that a whole buffer's worth of movement is spread rather than
/// stepped, short enough that a fader still feels attached to the hand — half
/// a buffer at 44.1 kHz, so a move is behind by less than one frame of video.
const TAU_SECONDS: f32 = 0.010;

/// Below this a gain is taken as arrived, so a fader at zero is silence rather
/// than an exponential tail. −140 dB: nothing, and reached in a few
/// milliseconds anyway.
const ARRIVED: f32 = 1e-7;

/// One value, following another.
#[derive(Debug, Clone, Copy)]
pub struct Smoothed {
    at: f32,
    /// How much of the gap survives each frame: `exp(-1 / (tau * rate))`.
    keep: f32,
}

impl Smoothed {
    /// Starting where it is asked for, so the first buffer is not a fade in
    /// from zero that nobody asked for.
    pub fn new(at: f32, sample_rate: u32) -> Self {
        // A device rate, so it fits a float exactly; clamped because a zero
        // would divide by nothing and a nonsense one would make the smoothing
        // either instant or eternal.
        let rate = sample_rate.clamp(8_000, 384_000) as f32;
        Self { at, keep: (-1.0 / (TAU_SECONDS * rate)).exp() }
    }

    /// The gain for one frame, having moved it towards `target`.
    pub fn step(&mut self, target: f32) -> f32 {
        let gap = target - self.at;
        if gap.abs() < ARRIVED {
            self.at = target;
        } else {
            self.at = target - gap * self.keep;
        }
        self.at
    }

    /// Where it is now, without moving it.
    #[cfg_attr(not(test), allow(dead_code, reason = "read by the smoothing tests"))]
    pub fn value(self) -> f32 {
        self.at
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "a fader at zero is silence exactly, and that is the assertion")]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    #[test]
    fn it_starts_where_it_is_told_rather_than_at_zero() {
        let mut gain = Smoothed::new(1.0, RATE);
        assert!((gain.step(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_jump_is_spread_over_about_ten_milliseconds() {
        // The definition of the time constant: after tau, most of the gap is
        // gone. Not all of it — this is an approach, not a ramp.
        let mut gain = Smoothed::new(1.0, RATE);
        let frames = (TAU_SECONDS * RATE as f32) as usize;
        for _ in 0..frames {
            gain.step(0.0);
        }
        let left = gain.value();
        assert!(left < 0.4 && left > 0.3, "after one tau the gain was {left}");
    }

    #[test]
    fn no_frame_moves_far_enough_to_be_heard_as_a_step() {
        // A fader slammed from silence to full: the largest single step must
        // stay well under what an ear hears as a click, which is what the
        // whole of this is for.
        let mut gain = Smoothed::new(0.0, RATE);
        let mut last = 0.0;
        let mut worst = 0.0_f32;
        for _ in 0..RATE as usize / 10 {
            let now = gain.step(1.0);
            worst = worst.max((now - last).abs());
            last = now;
        }
        assert!(worst < 0.01, "the largest step was {worst}");
        assert!(last > 0.99, "it never arrived: {last}");
    }

    #[test]
    fn it_settles_exactly_rather_than_leaving_a_tail() {
        let mut gain = Smoothed::new(1.0, RATE);
        for _ in 0..RATE as usize {
            gain.step(0.0);
        }
        assert_eq!(gain.value(), 0.0);
    }

    #[test]
    fn a_target_that_keeps_moving_is_trailed_by_a_fixed_distance() {
        // A hand dragging a fader steadily: the value trails the target by one
        // time constant's worth of the movement and no more. That the lag is
        // constant rather than growing is the whole property — a smoother that
        // fell further behind the longer a drag went on would be unusable.
        let mut gain = Smoothed::new(0.0, RATE);
        let frames = RATE as usize / 10;
        let mut lag_at_half = 0.0;
        for i in 0..frames {
            let target = i as f32 / frames as f32;
            let at = gain.step(target);
            if i == frames / 2 {
                lag_at_half = target - at;
            }
        }
        let lag_at_end = 1.0 - gain.value();
        // tau times the slope: 10 ms of a move that takes 100 ms.
        let expected = TAU_SECONDS / 0.1;
        assert!((lag_at_half - expected).abs() < 0.01, "half way it lagged {lag_at_half}");
        assert!((lag_at_end - expected).abs() < 0.01, "at the end it lagged {lag_at_end}");
    }
}
