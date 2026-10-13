//! The master limiter: a lookahead peak limiter on the sum of both decks.
//!
//! Two decks at full level sum past 1.0, and a clamp there is a flat top on
//! every kick — that is the distortion heard mixing two mastered tracks with
//! both faders up. A limiter turns the sum down for the few milliseconds the
//! peak lasts and lets it back up afterwards, which is what the master section
//! of a DJM does with its own.
//!
//! Lookahead rather than a fast attack: the audio is delayed by a millisecond
//! and a half and the gain is worked out from what is *about* to arrive, so
//! the reduction is in place when the peak reaches the output rather than a
//! few samples after it. A limiter without lookahead lets the first cycle of
//! every transient through and clips it anyway.
//!
//! The chain is the textbook one and every stage is bounded by construction:
//!
//! 1. Each frame asks for a gain, `ceiling / peak`, with the two channels
//!    linked on the louder — a limiter that treats them apart moves the image.
//! 2. A sliding minimum over the last `lookahead + 1` frames, so the gain for
//!    any frame is never more than the lowest asked for in its neighbourhood.
//! 3. A release: the gain follows that minimum down at once and back up along
//!    an exponential, so a run of peaks is one dip rather than a flutter.
//! 4. A box filter `lookahead` frames long, which turns the drop into a ramp
//!    arriving at its floor exactly as the peak leaves the delay line.
//!
//! Stage 4 averages `lookahead` values of stage 3, each of which is at most the
//! minimum over a window that includes the frame coming out of the delay — so
//! the gain applied is never more than that frame asked for, and nothing over
//! the ceiling leaves here. The clamp after it in the callback is a belt for
//! this pair of braces, and rounding aside it never engages.
//!
//! Nothing here allocates or locks once built. The interface writes settings
//! as atomics and the callback reads them once a buffer.

use crate::smooth::Smoothed;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub const MIN_INPUT_GAIN_DB: f32 = -24.0;
pub const MAX_INPUT_GAIN_DB: f32 = 24.0;
// Leave 4 dB of summing headroom: two full-scale signals need about 2 dB of limiting.
pub const DEFAULT_INPUT_GAIN_DB: f32 = -4.0;

/// How far ahead the limiter looks, in seconds.
///
/// 1.5 ms: enough to ramp into a kick's transient without a step, and short
/// enough that the delay it costs the output is well under anything a hand on
/// a jog could feel. A DJM's own is not documented; this is the figure most
/// software limiters settle on.
const LOOKAHEAD_SECONDS: f32 = 0.0015;

/// The ceiling's range, in dBFS. Nothing above 0: that is the clamp's job.
pub const MIN_CEILING_DB: f32 = -12.0;
pub const MAX_CEILING_DB: f32 = 0.0;
/// Where a new limiter sits.
pub const DEFAULT_CEILING_DB: f32 = 0.0;

/// The release's range, in milliseconds.
///
/// Under ten is a limiter that follows the waveform itself and distorts on
/// bass; over a second is one that ducks a whole bar for one kick.
pub const MIN_RELEASE_MS: f32 = 10.0;
pub const MAX_RELEASE_MS: f32 = 1_000.0;
pub const DEFAULT_RELEASE_MS: f32 = 250.0;

/// What the interface can set. One atomic each, read once a callback.
#[derive(Debug)]
pub struct LimiterSettings {
    enabled: AtomicBool,
    input_gain_db: AtomicU32,
    ceiling_db: AtomicU32,
    release_ms: AtomicU32,
}

impl Default for LimiterSettings {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            input_gain_db: AtomicU32::new(DEFAULT_INPUT_GAIN_DB.to_bits()),
            ceiling_db: AtomicU32::new(DEFAULT_CEILING_DB.to_bits()),
            release_ms: AtomicU32::new(DEFAULT_RELEASE_MS.to_bits()),
        }
    }
}

impl LimiterSettings {
    pub fn input_gain_db(&self) -> f32 {
        f32::from_bits(self.input_gain_db.load(Ordering::Relaxed))
    }

    pub fn set_input_gain_db(&self, db: f32) {
        let safe = if db.is_finite() {
            db.clamp(MIN_INPUT_GAIN_DB, MAX_INPUT_GAIN_DB)
        } else {
            DEFAULT_INPUT_GAIN_DB
        };
        self.input_gain_db.store(safe.to_bits(), Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Off is not a bypass: the gain releases to unity over the release time
    /// and the delay stays, so a toggle mid-track is not a splice.
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn ceiling_db(&self) -> f32 {
        f32::from_bits(self.ceiling_db.load(Ordering::Relaxed))
    }

    /// The ceiling, in dBFS. Out of range is clamped and a NaN is the default.
    pub fn set_ceiling_db(&self, db: f32) {
        let safe = if db.is_finite() {
            db.clamp(MIN_CEILING_DB, MAX_CEILING_DB)
        } else {
            DEFAULT_CEILING_DB
        };
        self.ceiling_db.store(safe.to_bits(), Ordering::Relaxed);
    }

    pub fn release_ms(&self) -> f32 {
        f32::from_bits(self.release_ms.load(Ordering::Relaxed))
    }

    /// The release, in milliseconds. Out of range is clamped, a NaN the default.
    pub fn set_release_ms(&self, ms: f32) {
        let safe = if ms.is_finite() {
            ms.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS)
        } else {
            DEFAULT_RELEASE_MS
        };
        self.release_ms.store(safe.to_bits(), Ordering::Relaxed);
    }
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// The limiter's state for one stream. Built once the device's rate is known.
pub struct Limiter {
    input_gain: Smoothed,
    /// The delay line, interleaved stereo, `lookahead` frames long.
    delay: Vec<f32>,
    /// Where the next frame goes in — and so where the oldest comes out.
    delay_at: usize,
    /// The sliding minimum's window: pairs of (frame index, gain asked for),
    /// gains increasing from front to back so the front is the minimum.
    window: VecDeque<(u64, f32)>,
    /// How many frames the sliding minimum spans: `lookahead + 1`.
    window_len: u64,
    /// Frames seen so far, the index the window's entries carry.
    frame: u64,
    /// The released gain, before the attack ramp.
    released: f32,
    /// The box filter over `released`: its last `lookahead` values and their sum.
    ramp: Vec<f32>,
    ramp_at: usize,
    /// In f64 so an hour of adding and subtracting does not drift the gain;
    /// resummed from the buffer each time the ring wraps regardless.
    ramp_sum: f64,
    /// `lookahead` frames as a divisor.
    lookahead: usize,
    /// The device's rate, for turning a release time into a coefficient.
    rate: f32,
    /// The lowest gain applied since the last read, for the meter.
    floor: f32,
}

impl Limiter {
    pub fn new(sample_rate: u32) -> Self {
        // Clamped as `smooth` clamps: a zero rate would be a limiter with no
        // lookahead at all, and a nonsense one would be a buffer of nothing.
        let rate = sample_rate.clamp(8_000, 384_000);
        // Never under one frame, so the delay line and the ramp exist.
        let lookahead = ((LOOKAHEAD_SECONDS * rate as f32).round() as usize).max(1);
        Self {
            delay: vec![0.0; lookahead * 2],
            input_gain: Smoothed::new(1.0, rate),
            delay_at: 0,
            window: VecDeque::with_capacity(lookahead + 1),
            window_len: lookahead as u64 + 1,
            frame: 0,
            released: 1.0,
            ramp: vec![1.0; lookahead],
            ramp_at: 0,
            ramp_sum: lookahead as f64,
            lookahead,
            rate: rate as f32,
            floor: 1.0,
        }
    }

    /// How many frames the output lags the input.
    pub fn latency_frames(&self) -> usize {
        self.lookahead
    }

    /// Runs one buffer of interleaved stereo through, in place.
    ///
    /// The settings are read once here rather than per frame: a ceiling that
    /// changed in the middle of a buffer would be one of two ramps, not both.
    pub fn process(&mut self, out: &mut [f32], settings: &LimiterSettings) {
        let ceiling = db_to_gain(settings.ceiling_db());
        let release = settings.release_ms().clamp(MIN_RELEASE_MS, MAX_RELEASE_MS) / 1_000.0;
        // The fraction of the gap to unity closed each frame on the way up.
        let rise = 1.0 - (-1.0 / (release * self.rate)).exp();
        let enabled = settings.enabled();
        let input_target = if enabled {
            db_to_gain(settings.input_gain_db())
        } else {
            1.0
        };

        for frame in out.chunks_exact_mut(2) {
            let (Some(&left), Some(&right)) = (frame.first(), frame.get(1)) else {
                continue;
            };
            let input_gain = self.input_gain.step(input_target);
            let (left, right) = (left * input_gain, right * input_gain);

            // 1. What this frame asks for. Off asks for nothing, and the
            // release brings the gain home rather than snapping it there.
            let peak = left.abs().max(right.abs());
            let wanted = if !enabled || peak <= ceiling {
                1.0
            } else {
                ceiling / peak
            };

            // 2. The sliding minimum over the last `window_len` frames.
            while self.window.back().is_some_and(|&(_, gain)| gain >= wanted) {
                self.window.pop_back();
            }
            self.window.push_back((self.frame, wanted));
            let oldest = self.frame.saturating_sub(self.window_len - 1);
            while self.window.front().is_some_and(|&(at, _)| at < oldest) {
                self.window.pop_front();
            }
            let minimum = self.window.front().map_or(wanted, |&(_, gain)| gain);
            self.frame = self.frame.wrapping_add(1);

            // 3. Down at once, up along the release.
            self.released = if minimum < self.released {
                minimum
            } else {
                self.released + (minimum - self.released) * rise
            };

            // 4. The box filter that makes the drop a ramp.
            let leaving = self.ramp.get(self.ramp_at).copied().unwrap_or(1.0);
            self.ramp_sum += f64::from(self.released) - f64::from(leaving);
            if let Some(slot) = self.ramp.get_mut(self.ramp_at) {
                *slot = self.released;
            }
            self.ramp_at += 1;
            if self.ramp_at >= self.lookahead {
                self.ramp_at = 0;
                self.ramp_sum = self.ramp.iter().map(|&g| f64::from(g)).sum();
            }
            // Never above unity: the average of gains that are each at most 1,
            // rounding aside.
            let gain = ((self.ramp_sum / self.lookahead as f64) as f32).min(1.0);
            self.floor = self.floor.min(gain);

            // The delay line: this frame goes in, the oldest comes out with
            // the gain that was worked out for it.
            let slot = self.delay_at * 2;
            let (delayed_left, delayed_right) = (
                self.delay.get(slot).copied().unwrap_or(0.0),
                self.delay.get(slot + 1).copied().unwrap_or(0.0),
            );
            if let Some(cell) = self.delay.get_mut(slot) {
                *cell = left;
            }
            if let Some(cell) = self.delay.get_mut(slot + 1) {
                *cell = right;
            }
            self.delay_at += 1;
            if self.delay_at >= self.lookahead {
                self.delay_at = 0;
            }

            if let Some(cell) = frame.first_mut() {
                *cell = delayed_left * gain;
            }
            if let Some(cell) = frame.get_mut(1) {
                *cell = delayed_right * gain;
            }
        }
    }

    /// The lowest gain applied since the last call, and a reset: the meter's
    /// read of how hard the limiter has been working.
    pub fn take_floor(&mut self) -> f32 {
        std::mem::replace(&mut self.floor, 1.0)
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "unity is unity exactly, and that is the assertion"
)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    #[test]
    fn defaults_only_shave_about_two_db_off_two_full_scale_tracks() {
        let settings = LimiterSettings::default();
        settings.set_enabled(true);
        let mut limiter = Limiter::new(RATE);
        // Allow input smoothing to settle before measuring the overlap.
        run(&mut limiter, &settings, &vec![(0.0, 0.0); RATE as usize]);
        let output = run(&mut limiter, &settings, &tone(2.0, RATE as usize));
        let reduction = -20.0 * limiter.take_floor().log10();
        assert!(
            (1.8..2.2).contains(&reduction),
            "defaults reduced {reduction} dB"
        );
        assert!(output
            .iter()
            .all(|&(l, r)| l.abs().max(r.abs()) <= 1.000_001));
    }

    #[test]
    fn input_gain_changes_level_before_limiting_and_bypasses_when_disabled() {
        for db in [-12.0, 6.0, 24.0] {
            let mut limiter = Limiter::new(RATE);
            let settings = LimiterSettings::default();
            settings.set_input_gain_db(0.0);
            settings.set_enabled(true);
            settings.set_input_gain_db(db);
            let input = vec![(0.25, 0.125); RATE as usize];
            let output = run(&mut limiter, &settings, &input);
            let expected = (0.25 * db_to_gain(db)).min(1.0);
            let (left, right) = output[output.len() - 1];
            assert!(
                (left - expected).abs() < 1e-4,
                "{db} dB gave {left}, expected {expected}"
            );
            assert!((right - left * 0.5).abs() < 1e-6);
            assert!(output.iter().all(|&(l, r)| l <= 1.000_001 && r <= 1.000_001));
            if db == 24.0 {
                assert!(limiter.take_floor() < 0.3);
            }
            settings.set_enabled(false);
            let bypassed = run(
                &mut limiter,
                &settings,
                &vec![(0.25, 0.125); RATE as usize * 4],
            );
            assert!((bypassed[bypassed.len() - 1].0 - 0.25).abs() < 1e-4);
        }
    }

    #[test]
    fn changing_input_gain_mid_stream_is_smoothed() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        let input = vec![(0.01, 0.01); 4_800];
        run(&mut limiter, &settings, &input);
        settings.set_input_gain_db(24.0);
        let output = run(&mut limiter, &settings, &input);
        assert!(output
            .windows(2)
            .all(|pair| (pair[1].0 - pair[0].0).abs() < 0.001));
        assert!(output[output.len() - 1].0 > 0.15);
    }

    fn run(
        limiter: &mut Limiter,
        settings: &LimiterSettings,
        frames: &[(f32, f32)],
    ) -> Vec<(f32, f32)> {
        let mut buffer: Vec<f32> = frames.iter().flat_map(|&(l, r)| [l, r]).collect();
        limiter.process(&mut buffer, settings);
        buffer.chunks_exact(2).map(|f| (f[0], f[1])).collect()
    }

    /// A tone at `amplitude`, `frames` long.
    fn tone(amplitude: f32, frames: usize) -> Vec<(f32, f32)> {
        (0..frames)
            .map(|i| {
                let s =
                    (i as f32 * 2.0 * std::f32::consts::PI * 100.0 / RATE as f32).sin() * amplitude;
                (s, s)
            })
            .collect()
    }

    #[test]
    fn a_new_limiter_looks_a_millisecond_and_a_half_ahead() {
        assert_eq!(Limiter::new(RATE).latency_frames(), 72);
        assert_eq!(Limiter::new(44_100).latency_frames(), 66);
        assert!(
            Limiter::new(0).latency_frames() >= 1,
            "a nonsense rate still delays"
        );
    }

    #[test]
    fn quiet_audio_passes_untouched_but_late() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        let input = tone(0.5, 1_000);
        let output = run(&mut limiter, &settings, &input);
        let delay = limiter.latency_frames();
        for (i, &(l, r)) in output.iter().enumerate().skip(delay) {
            let (el, er) = input[i - delay];
            assert!(
                (l - el).abs() < 1e-6 && (r - er).abs() < 1e-6,
                "frame {i} was changed"
            );
        }
        assert_eq!(limiter.take_floor(), 1.0, "nothing was reduced");
    }

    #[test]
    fn nothing_over_the_ceiling_leaves() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        let ceiling = db_to_gain(DEFAULT_CEILING_DB);
        // Two full-scale decks summed: the case that clipped.
        let input = tone(2.0, 4_800);
        let output = run(&mut limiter, &settings, &input);
        let loudest = output
            .iter()
            .map(|&(l, r)| l.abs().max(r.abs()))
            .fold(0.0, f32::max);
        assert!(
            loudest <= ceiling + 1e-6,
            "{loudest} is over the ceiling {ceiling}"
        );
        assert!(
            loudest > ceiling * 0.9,
            "{loudest}: the limiter is a fader, not a limiter"
        );
        assert!(
            limiter.take_floor() < 0.6,
            "six decibels over needs six off"
        );
    }

    #[test]
    fn a_single_peak_is_ramped_into_rather_than_stepped() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        let delay = limiter.latency_frames();
        // Silence, one full-scale-times-two sample, silence.
        let mut input = vec![(0.3, 0.3); 2_000];
        input[500] = (2.0, 2.0);
        let output = run(&mut limiter, &settings, &input);
        // The frame before the peak has already been turned down: the ramp
        // began a lookahead earlier.
        let before = output[500 + delay - 1].0;
        assert!(
            before < 0.3,
            "the gain had not started moving before the peak: {before}"
        );
        // The ramp is exactly a lookahead long: the frame before it begins is
        // untouched.
        let well_before = output[499].0;
        assert!(
            (well_before - 0.3).abs() < 1e-6,
            "the ramp started too early: {well_before}"
        );
        // And the peak itself lands at the ceiling.
        let at = output[500 + delay].0;
        assert!(
            at <= db_to_gain(DEFAULT_CEILING_DB) + 1e-6,
            "the peak got through: {at}"
        );
        // No step: consecutive frames of the ramp differ by a bounded amount.
        for pair in output[500..500 + delay].windows(2) {
            assert!(
                (pair[0].0 - pair[1].0).abs() < 0.02,
                "a step in the ramp: {pair:?}"
            );
        }
    }

    #[test]
    fn the_gain_comes_back_along_the_release() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        settings.set_release_ms(50.0);
        let mut input = tone(2.0, 480);
        input.extend(tone(0.3, 48_000));
        let output = run(&mut limiter, &settings, &input);
        // Twenty milliseconds after the loud part the tone is still held down;
        // half a second on it is back to what it was.
        let peak_around = |centre: usize| {
            output[centre - 200..centre + 200]
                .iter()
                .map(|&(l, _)| l.abs())
                .fold(0.0, f32::max)
        };
        let soon = peak_around(480 + 960);
        assert!(soon < 0.29, "no release yet: {soon}");
        let later = peak_around(480 + 24_000);
        assert!((later - 0.3).abs() < 1e-3, "released: {later}");
    }

    #[test]
    fn switched_off_it_releases_to_unity_and_lets_the_sum_through() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(false);
        let input = tone(1.5, 4_800);
        let output = run(&mut limiter, &settings, &input);
        let loudest = output.iter().map(|&(l, _)| l.abs()).fold(0.0, f32::max);
        assert!((loudest - 1.5).abs() < 1e-3, "off means off: {loudest}");
        assert_eq!(limiter.take_floor(), 1.0);
    }

    #[test]
    fn both_channels_move_together() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        // Loud on the left only: the right must be turned down the same.
        let input: Vec<(f32, f32)> = tone(2.0, 4_800)
            .into_iter()
            .map(|(l, _)| (l, l * 0.25))
            .collect();
        let output = run(&mut limiter, &settings, &input);
        for &(l, r) in output.iter().skip(2_400) {
            if l.abs() > 1e-3 {
                assert!((r / l - 0.25).abs() < 1e-4, "the image moved: {l} {r}");
            }
        }
    }

    #[test]
    fn settings_out_of_range_are_clamped_and_a_nan_is_the_default() {
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_input_gain_db(100.0);
        assert_eq!(settings.input_gain_db(), MAX_INPUT_GAIN_DB);
        settings.set_input_gain_db(-100.0);
        assert_eq!(settings.input_gain_db(), MIN_INPUT_GAIN_DB);
        settings.set_input_gain_db(f32::NAN);
        assert_eq!(settings.input_gain_db(), DEFAULT_INPUT_GAIN_DB);
        settings.set_ceiling_db(3.0);
        assert_eq!(settings.ceiling_db(), MAX_CEILING_DB);
        settings.set_ceiling_db(-40.0);
        assert_eq!(settings.ceiling_db(), MIN_CEILING_DB);
        settings.set_ceiling_db(f32::NAN);
        assert_eq!(settings.ceiling_db(), DEFAULT_CEILING_DB);
        settings.set_release_ms(0.0);
        assert_eq!(settings.release_ms(), MIN_RELEASE_MS);
        settings.set_release_ms(f32::INFINITY);
        assert_eq!(settings.release_ms(), DEFAULT_RELEASE_MS);
    }

    /// The tests/engine.rs measure, restated here: steps that stand out from
    /// the two milliseconds either side of them by more than twelve times.
    fn clicks(frames: &[(f32, f32)]) -> usize {
        const NEIGHBOURHOOD: usize = 96;
        const CLICK_RATIO: f32 = 12.0;
        let steps: Vec<f32> = frames
            .windows(2)
            .map(|pair| (pair[1].0 - pair[0].0).abs())
            .collect();
        (NEIGHBOURHOOD..steps.len().saturating_sub(NEIGHBOURHOOD))
            .filter(|&i| {
                let around: f32 = steps[i - NEIGHBOURHOOD..i]
                    .iter()
                    .chain(&steps[i + 1..i + 1 + NEIGHBOURHOOD])
                    .sum::<f32>()
                    / (NEIGHBOURHOOD * 2) as f32;
                around > 1e-4 && steps[i] > around * CLICK_RATIO
            })
            .count()
    }

    #[test]
    fn a_ceiling_lowered_mid_stream_holds_from_a_lookahead_on() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        let delay = limiter.latency_frames();
        // Settled under the default ceiling, then the ceiling drops by six.
        run(&mut limiter, &settings, &tone(2.0, 4_800));
        settings.set_ceiling_db(-6.0);
        let lower = db_to_gain(-6.0);
        let output = run(&mut limiter, &settings, &tone(2.0, 4_800));
        // The frames still in the delay line were let through under the old
        // ceiling; every frame that entered under the new one leaves under it.
        let loudest = output
            .iter()
            .skip(delay)
            .map(|&(l, r)| l.abs().max(r.abs()))
            .fold(0.0, f32::max);
        assert!(
            loudest <= lower + 1e-6,
            "{loudest} is over the new ceiling {lower}"
        );
        assert!(
            loudest > lower * 0.9,
            "{loudest}: the new ceiling is not being reached"
        );
    }

    #[test]
    fn switching_on_over_a_loud_signal_ramps_rather_than_steps() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(false);
        let lookahead = limiter.latency_frames();
        let amplitude = 2.0;
        let mut output = run(&mut limiter, &settings, &tone(amplitude, 4_800));
        settings.set_enabled(true);
        output.extend(run(&mut limiter, &settings, &tone(amplitude, 4_800)));
        // A straight switch would step by the whole reduction at once. The
        // ramp spreads it over the lookahead, so no step is larger than the
        // tone's own plus that share of the reduction.
        let floor = db_to_gain(DEFAULT_CEILING_DB) / amplitude;
        let tone_step = amplitude * 2.0 * std::f32::consts::PI * 100.0 / RATE as f32;
        let allowed = tone_step + amplitude * (1.0 - floor) / lookahead as f32;
        let largest = output
            .windows(2)
            .map(|pair| (pair[1].0 - pair[0].0).abs())
            .fold(0.0, f32::max);
        assert!(
            largest <= allowed + 1e-4,
            "a step of {largest} where the ramp allows {allowed}"
        );
        assert_eq!(clicks(&output), 0);
        // And it did switch on: the tail is at the ceiling, not at two.
        let tail = output[8_000..]
            .iter()
            .map(|&(l, _)| l.abs())
            .fold(0.0, f32::max);
        assert!(
            tail <= db_to_gain(DEFAULT_CEILING_DB) + 1e-6,
            "still off: {tail}"
        );
    }

    #[test]
    fn the_floor_is_reset_by_reading_it() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        settings.set_release_ms(100.0);
        run(&mut limiter, &settings, &tone(2.0, 4_800));
        let loud = limiter.take_floor();
        assert!(loud < 0.6, "the loud buffer was not measured: {loud}");
        // Nothing processed since: the reading is unity, not the old floor.
        assert_eq!(limiter.take_floor(), 1.0);
        // A quiet second after the loud one measures as its own: the release
        // starts it under unity, and nothing in it asks for less than the
        // loud buffer did.
        run(&mut limiter, &settings, &tone(0.1, RATE as usize));
        let after = limiter.take_floor();
        assert!(after < 1.0, "the release had not started: {after}");
        assert!(
            after >= loud - 1e-6,
            "quiet audio was reduced further: {after} under {loud}"
        );
        // And once the release has run its course, a quiet buffer reads as
        // unity to within the exponential's tail.
        run(&mut limiter, &settings, &tone(0.1, 480));
        let released = limiter.take_floor();
        assert!(released > 0.999, "the release never finished: {released}");
    }

    #[test]
    fn a_silent_channel_is_turned_down_with_its_loud_partner() {
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        // Loud on the left, nothing on the right: linked on the louder, so
        // the left is held to the ceiling, and the right stays at nothing.
        let input: Vec<(f32, f32)> = tone(2.0, 4_800)
            .into_iter()
            .map(|(l, _)| (l, 0.0))
            .collect();
        let output = run(&mut limiter, &settings, &input);
        let ceiling = db_to_gain(DEFAULT_CEILING_DB);
        let loudest_left = output.iter().map(|&(l, _)| l.abs()).fold(0.0, f32::max);
        assert!(
            loudest_left <= ceiling + 1e-6,
            "{loudest_left} is over the ceiling"
        );
        assert!(
            loudest_left > ceiling * 0.9,
            "{loudest_left}: the left was not limited to the ceiling"
        );
        assert!(
            output.iter().all(|&(_, r)| r == 0.0),
            "silence came out as something"
        );
        assert!(
            limiter.take_floor() < 0.6,
            "the left's peak did not drive the gain"
        );
    }

    #[test]
    fn a_long_run_does_not_drift_the_gain() {
        // The box filter's running sum, resummed each wrap: an hour of quiet
        // audio should still be unity to the last bit.
        let mut limiter = Limiter::new(RATE);
        let settings = LimiterSettings::default();
        settings.set_input_gain_db(0.0);
        settings.set_enabled(true);
        let mut buffer = vec![0.1_f32; 512];
        for _ in 0..(RATE as usize * 60 / 256) {
            buffer.fill(0.1);
            limiter.process(&mut buffer, &settings);
        }
        assert!(
            buffer.iter().all(|&s| (s - 0.1).abs() < 1e-7),
            "drifted: {}",
            buffer[300]
        );
    }
}
