//! Where a kick's attack is, to the sample.
//!
//! The grid goes on the attack of the kick. The attack is the start of an
//! RMS spike in the 900–9000 Hz band: the click at the front of a kick,
//! which the kick's body (below 200 Hz) and the bass line do not have, and
//! which a hat between the beats does not have at the same instant as a
//! thump. The onset envelope the tempo stage works from places a beat to
//! within a 5.8 ms hop; this places it to a millisecond, from the audio.
//!
//! [`AttackMap`] band-passes the whole track once and keeps its RMS in
//! 1 ms steps, so that asking where the attack nearest a beat is costs a
//! scan of a few dozen steps.

/// What the attack finder is tuned by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackOptions {
    /// The band the click lives in.
    pub low_hz: f32,
    pub high_hz: f32,
    /// The RMS step, in seconds.
    pub step_secs: f64,
    /// How far either side of a predicted beat an attack is looked for.
    /// The grid that asks is already within a few milliseconds of the hit;
    /// 50 ms reached the clap after the kick on one golden track and the
    /// fit chased it, and 15 ms scores one grid better than 20 or 30.
    pub reach_secs: f64,
}

impl Default for AttackOptions {
    fn default() -> Self {
        Self { low_hz: 900.0, high_hz: 9000.0, step_secs: 0.001, reach_secs: 0.015 }
    }
}

/// A kick's attack: when, and how big a spike it started.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attack {
    pub secs: f64,
    /// The rise of the spike above the level before it, in RMS units of
    /// the band-passed signal.
    pub height: f64,
}

/// The track band-passed to the click band, kept as its RMS in steps.
#[derive(Debug, Clone)]
pub struct AttackMap {
    rms: Vec<f32>,
    step: usize,
    sample_rate: u32,
    options: AttackOptions,
}

impl AttackMap {
    /// Band-passes the track and takes its RMS in steps. Costs one pass
    /// over the samples.
    pub fn new(samples: &[f32], sample_rate: u32, options: AttackOptions) -> Self {
        let step = ((options.step_secs * f64::from(sample_rate)) as usize).max(1);
        let band = band_pass(samples, sample_rate, options.low_hz, options.high_hz);
        let rms: Vec<f32> = band
            .chunks(step)
            .map(|chunk| (chunk.iter().map(|x| x * x).sum::<f32>() / chunk.len().max(1) as f32).sqrt())
            .collect();
        Self { rms, step, sample_rate, options }
    }

    pub fn options(&self) -> AttackOptions {
        self.options
    }

    /// The attack nearest `secs`, within the options' reach, or `None` when
    /// nothing in the band rises there — a breakdown, a beatless intro.
    pub fn attack_near(&self, secs: f64) -> Option<Attack> {
        self.attack_within(secs, self.options.reach_secs)
    }

    /// The attack nearest `secs` within `reach_secs` either side.
    pub fn attack_within(&self, secs: f64, reach_secs: f64) -> Option<Attack> {
        if self.rms.len() < 3 || self.sample_rate == 0 {
            return None;
        }
        let step_secs = self.step as f64 / f64::from(self.sample_rate);
        let centre = (secs / step_secs).round().max(0.0) as usize;
        let reach = (reach_secs / step_secs).ceil().max(1.0) as usize;
        let lo = centre.saturating_sub(reach).max(1);
        let hi = (centre + reach).min(self.rms.len() - 1);
        if hi <= lo {
            return None;
        }
        // The steepest rise in the window is a spike's front — but the
        // spike wanted is the one nearest the asked-for time, so among the
        // rises at least half as steep as the steepest, the nearest wins.
        // Taking the steepest outright let a grid drift: once a beat's
        // prediction slips late, the window reaches a sharper hit further
        // on, the line follows it, and the next prediction slips further.
        let rise = |i: usize| f64::from(self.rms[i]) - f64::from(self.rms[i - 1]);
        let mut steepest = (0.0_f64, lo);
        for i in lo..=hi {
            let r = rise(i);
            if r > steepest.0 {
                steepest = (r, i);
            }
        }
        if steepest.0 <= 0.0 {
            return None;
        }
        // A rise that is no more than the window's ordinary wobble is not
        // a spike: it must stand well above the median rise in the window.
        let mut rises: Vec<f64> = (lo..=hi).map(|i| rise(i).abs()).collect();
        rises.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let ordinary = rises.get(rises.len() / 2).copied().unwrap_or(0.0);
        if steepest.0 < ordinary * MIN_SPIKE_OVER_WOBBLE {
            return None;
        }
        let strong = steepest.0 * NEAREST_STRONG_FRACTION;
        let mut chosen = steepest;
        for i in lo..=hi {
            let r = rise(i);
            if r >= strong && i.abs_diff(centre) < chosen.1.abs_diff(centre) {
                chosen = (r, i);
            }
        }
        let steepest = chosen;
        // Walk back from the steepest step to where the spike starts: the
        // first step of the run of rises leading into it.
        let mut start = steepest.1;
        while start > lo && rise(start - 1) > steepest.0 * RUN_FRACTION {
            start -= 1;
        }
        // The spike's height: its top within the window against the level
        // just before it starts.
        let floor = f64::from(self.rms[start - 1]);
        let top = (start..=hi).map(|i| f64::from(self.rms[i])).fold(floor, f64::max);
        // The attack begins inside the bin before the rise, not at that
        // bin's left edge. Its midpoint avoids a systematic half-bin early
        // bias that otherwise carries into every fitted beat.
        let sample = (start - 1) as f64 * self.step as f64 + self.step as f64 / 2.0;
        Some(Attack { secs: sample / f64::from(self.sample_rate), height: top - floor })
    }
}

/// A spike's steepest step must be this many times the window's median
/// step-to-step change to count.
const MIN_SPIKE_OVER_WOBBLE: f64 = 4.0;
/// A step still belongs to the spike's run while its rise is at least this
/// share of the steepest one.
const RUN_FRACTION: f64 = 0.1;
/// A rise at least this share of the window's steepest is a spike in its
/// own right, and the one nearest the asked-for time is taken.
const NEAREST_STRONG_FRACTION: f64 = 0.5;

/// Second-order high-pass then low-pass (Butterworth Q), direct form I.
fn band_pass(samples: &[f32], sample_rate: u32, low_hz: f32, high_hz: f32) -> Vec<f32> {
    let nyquist = sample_rate as f32 / 2.0;
    let high_pass = Biquad::high_pass(low_hz.clamp(1.0, nyquist * 0.99), sample_rate);
    let low_pass = Biquad::low_pass(high_hz.clamp(1.0, nyquist * 0.99), sample_rate);
    let mut hp = high_pass;
    let mut lp = low_pass;
    samples.iter().map(|&x| lp.run(hp.run(x))).collect()
}

/// One biquad section with its state.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn from_coefficients(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    /// RBJ cookbook high-pass, Q = 1/√2.
    fn high_pass(hz: f32, sample_rate: u32) -> Self {
        let w = 2.0 * std::f32::consts::PI * hz / sample_rate as f32;
        let alpha = w.sin() / (2.0 * std::f32::consts::FRAC_1_SQRT_2.recip());
        let c = w.cos();
        let half = 1.0_f32.midpoint(c);
        Self::from_coefficients(half, -(1.0 + c), half, 1.0 + alpha, -2.0 * c, 1.0 - alpha)
    }

    /// RBJ cookbook low-pass, Q = 1/√2.
    pub(crate) fn low_pass(hz: f32, sample_rate: u32) -> Self {
        let w = 2.0 * std::f32::consts::PI * hz / sample_rate as f32;
        let alpha = w.sin() / (2.0 * std::f32::consts::FRAC_1_SQRT_2.recip());
        let c = w.cos();
        let half = 1.0_f32.midpoint(-c);
        Self::from_coefficients(half, 1.0 - c, half, 1.0 + alpha, -2.0 * c, 1.0 - alpha)
    }

    pub(crate) fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}
