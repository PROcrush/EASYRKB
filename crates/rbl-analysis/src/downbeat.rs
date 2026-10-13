//! Which beat is the downbeat, and whether the beat is where the grid says.
//!
//! The tempo stage numbers beats 1..=4 from the first one it found, which is
//! a claim about the beat and not about the bar — and its beat can be half a
//! beat off, because in a bar of tech house the open hat between the kicks
//! carries as much spectral flux as the kick, and in a bar of hardstyle the
//! reverse bass does. No band of the spectrum tells the kick from the
//! off-beat on every track; measured on the golden playlist, the full band
//! follows the hats on tech house and the low band follows the bass on
//! hardstyle.
//!
//! Structure does. Dance music changes at bar boundaries and, far more
//! strongly, at phrase boundaries eight or sixteen bars apart — a drop, a
//! breakdown, a new bass line, a filter opening — and every such change
//! lands on a downbeat, which is a kick, never on the off-beat. So the track
//! is summarised as one spectral profile per *half* beat, a novelty is
//! measured at every half beat as the distance between the profiles of the
//! bars before it and after it (at several scales, from one bar to eight),
//! and the eight positions in the bar are ranked by the novelty peaks that
//! land on them. The winning position says both which half of the beat the
//! music changes on and which beat of the bar.
//!
//! Only peaks of the novelty count. Summing the novelty itself put nearly
//! equal weight on every position (a margin of 3 % between the best and the
//! next), because a slow crescendo raises every half beat alike; keeping
//! local maxima only raises the margin to 2.8× and picks rekordbox's own
//! downbeat on its own grid for 153 of the 155 golden tracks.

use realfft::RealFftPlanner;

/// Frame and hop of the spectral profiles: ~46 ms and ~12 ms at 44.1 kHz,
/// coarse enough that a beat holds several frames and cheap enough that a
/// whole track costs tens of milliseconds.
const FRAME: usize = 2048;
const HOP: usize = 512;

/// The band edges of a profile, in hertz. Log-spaced over the range that
/// carries structure: the kick and bass at the bottom, hats at the top.
const BAND_EDGES: [f64; 13] =
    [40.0, 65.0, 100.0, 160.0, 250.0, 400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0, 10000.0];
pub const BANDS: usize = BAND_EDGES.len() - 1;

/// The bar lengths novelty is measured over, in half beats: one, two, four
/// and eight bars. One bar catches the bass line changing; eight catches
/// the drop.
const SCALES: [usize; 4] = [8, 16, 32, 64];

/// Minimum four-bar change in the Euclidean distance of mean log-band
/// energies. Below this, FFT alignment noise must not decide bar position.
const MIN_STRUCTURE_NOVELTY: f64 = 0.5;

/// Half beats in a bar.
const POSITIONS: usize = 8;

/// Beats a tempo segment needs before its bar position is decided from its
/// own music rather than carried over: eight bars, enough for a four-bar
/// comparison on both sides. Shorter stretches carry the chosen count.
pub const MIN_BEATS_FOR_OWN_PHASE: usize = 32;

/// Where the grid sits in the bar.
#[derive(Debug, Clone, PartialEq)]
pub struct GridPhase {
    /// The music's beats are half a beat after the grid's: the grid was
    /// built on the off-beat and must move.
    pub half_beat_off: bool,
    /// A downbeat, in seconds from the start of the file. A time rather
    /// than an index into the grid: moving the grid by half a beat can put
    /// a new first beat before the old one and renumber everything.
    pub downbeat_secs: f64,
    /// Where phrases start, in seconds, in order: the downbeats at which
    /// the music changes most over four bars either side.
    pub phrase_starts: Vec<f64>,
}

/// Decides where the grid's beats sit in the bar, and whether they sit on
/// the beat at all.
///
/// `beat_secs` is every beat's time in seconds, in order. A track too short
/// to have a bar in it comes back unshifted with the first beat a downbeat.
pub fn grid_phase(samples: &[f32], sample_rate: u32, beat_secs: &[f64]) -> GridPhase {
    grid_phase_from(&band_frames(samples, sample_rate), sample_rate, beat_secs)
}

/// [`grid_phase`] over band frames already taken from the track with
/// [`band_frames`], which cost most of the stage and do not depend on the
/// grid, so a caller can take them while the grid is still being found.
pub fn grid_phase_from(frames: &[[f64; BANDS]], sample_rate: u32, beat_secs: &[f64]) -> GridPhase {
    let unshifted =
        GridPhase { half_beat_off: false, downbeat_secs: beat_secs.first().copied().unwrap_or(0.0), phrase_starts: Vec::new() };
    if beat_secs.len() < 8 || frames.is_empty() || sample_rate == 0 {
        return unshifted;
    }
    // The half-beat grid: every beat and the midpoint after it.
    let mut halves = Vec::with_capacity(beat_secs.len() * 2);
    for pair in beat_secs.windows(2) {
        halves.push(pair[0]);
        halves.push(pair[0].midpoint(pair[1]));
    }
    let profiles = beat_profiles_from(frames, sample_rate, &halves);
    if profiles.len() < POSITIONS * 2 {
        return unshifted;
    }
    // Identical repeated beats have no bar-position evidence. FFT frame
    // boundaries still create small spectral fluctuations; normalizing
    // those to unit weight otherwise invents a later downbeat. Require
    // an actual change over four bars before overriding the first beat.
    if novelty_peaks(&profiles, 32).iter().all(|&(_, value)| value < MIN_STRUCTURE_NOVELTY) {
        return unshifted;
    }
    let scores = position_scores(&profiles, POSITIONS, &SCALES);
    // The half-beat position, counted from the grid's first beat, that the
    // music most often changes on: the downbeat.
    let best = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i);
    // An odd position is a midpoint: the beats are there, not on the grid.
    // The downbeat is that position itself, as a time.
    GridPhase {
        half_beat_off: best % 2 == 1,
        downbeat_secs: halves.get(best).copied().unwrap_or(0.0),
        phrase_starts: phrase_starts(&profiles, &halves, best),
    }
}

/// Where phrases start: the half-beat positions, on the downbeat, at which
/// the music changes most at the four-bar scale.
///
/// `halves` is the half-beat grid the profiles were taken on and
/// `downbeat` the position in `0..8` that is beat 1. A start is a novelty
/// peak on a downbeat at least `MIN_PHRASE_PEAK` of the strongest peak.
/// Returned as times in seconds, in order.
pub fn phrase_starts(profiles: &[[f64; BANDS]], halves: &[f64], downbeat: usize) -> Vec<f64> {
    const SCALE: usize = 32;
    let peaks = novelty_peaks(profiles, SCALE);
    let strongest = peaks.iter().map(|&(_, v)| v).fold(0.0_f64, f64::max);
    peaks
        .iter()
        .filter(|&&(i, v)| i % POSITIONS == downbeat && v >= strongest * MIN_PHRASE_PEAK)
        .filter_map(|&(i, _)| halves.get(i).copied())
        .collect()
}

/// A novelty peak counts as a phrase start from this share of the
/// strongest peak in the track.
const MIN_PHRASE_PEAK: f64 = 0.3;

/// The novelty at every slot at one scale, with everything but local
/// maxima set to zero, as `(slot, novelty)` for the maxima.
fn novelty_peaks(profiles: &[[f64; BANDS]], scale: usize) -> Vec<(usize, f64)> {
    let n = profiles.len();
    if n < scale * 2 {
        return Vec::new();
    }
    let mut prefix = vec![[0.0_f64; BANDS]; n + 1];
    for (i, profile) in profiles.iter().enumerate() {
        for b in 0..BANDS {
            prefix[i + 1][b] = prefix[i][b] + profile[b];
        }
    }
    let mean_over = |from: usize, to: usize| -> [f64; BANDS] {
        let mut out = [0.0_f64; BANDS];
        let len = (to - from) as f64;
        for b in 0..BANDS {
            out[b] = (prefix[to][b] - prefix[from][b]) / len;
        }
        out
    };
    let mut novelty = vec![0.0_f64; n];
    for (i, slot) in novelty.iter_mut().enumerate().take(n - scale + 1).skip(scale) {
        let before = mean_over(i - scale, i);
        let after = mean_over(i, i + scale);
        let mut d = 0.0;
        for b in 0..BANDS {
            let diff = after[b] - before[b];
            d += diff * diff;
        }
        *slot = d.sqrt();
    }
    (0..n)
        .filter(|&i| {
            let left = if i > 0 { novelty[i - 1] } else { 0.0 };
            let right = novelty.get(i + 1).copied().unwrap_or(0.0);
            novelty[i] > left && novelty[i] >= right && novelty[i] > 0.0
        })
        .map(|i| (i, novelty[i]))
        .collect()
}

/// How much novelty lands on each of `positions` slots per bar, when the
/// profiles are one per slot: with four slots a profile is a beat, with
/// eight it is half a beat. `scales` are bar lengths in slots.
pub fn position_scores(profiles: &[[f64; BANDS]], positions: usize, scales: &[usize]) -> Vec<f64> {
    let n = profiles.len();
    let mut scores = vec![0.0_f64; positions.max(1)];
    for &scale in scales {
        if n < scale * 2 {
            continue;
        }
        // The distance between the mean profile over `scale` beats before
        // beat i and over `scale` beats from i. Prefix sums make each of
        // those a subtraction.
        let mut prefix = vec![[0.0_f64; BANDS]; n + 1];
        for (i, profile) in profiles.iter().enumerate() {
            for b in 0..BANDS {
                prefix[i + 1][b] = prefix[i][b] + profile[b];
            }
        }
        let mean_over = |from: usize, to: usize| -> [f64; BANDS] {
            let mut out = [0.0_f64; BANDS];
            let len = (to - from) as f64;
            for b in 0..BANDS {
                out[b] = (prefix[to][b] - prefix[from][b]) / len;
            }
            out
        };
        let mut novelty = vec![0.0_f64; n];
        for (i, slot) in novelty.iter_mut().enumerate().take(n - scale + 1).skip(scale) {
            let before = mean_over(i - scale, i);
            let after = mean_over(i, i + scale);
            let mut d = 0.0;
            for b in 0..BANDS {
                let diff = after[b] - before[b];
                d += diff * diff;
            }
            *slot = d.sqrt();
        }
        // Only a slot whose novelty stands out among its neighbours says
        // anything about the bar: a slow crescendo raises every slot alike.
        // Summing the novelty itself put nearly equal weight on every
        // position; keeping the peaks alone is what makes the choice sharp.
        let raw = novelty.clone();
        for (i, value) in novelty.iter_mut().enumerate() {
            let left = if i > 0 { raw[i - 1] } else { 0.0 };
            let right = raw.get(i + 1).copied().unwrap_or(0.0);
            if !(raw[i] > left && raw[i] >= right) {
                *value = 0.0;
            }
        }
        // Each scale contributes equally, so a long phrase counts as much as
        // a bar.
        let total: f64 = novelty.iter().sum();
        if total <= 0.0 {
            continue;
        }
        for (i, value) in novelty.iter().enumerate() {
            scores[i % positions.max(1)] += value / total;
        }
    }
    scores
}

/// One spectral profile per beat: the mean log energy of each band over the
/// frames the beat spans.
pub fn beat_profiles(samples: &[f32], sample_rate: u32, beat_secs: &[f64]) -> Vec<[f64; BANDS]> {
    beat_profiles_from(&band_frames(samples, sample_rate), sample_rate, beat_secs)
}

/// [`beat_profiles`] over frames already taken with [`band_frames`].
pub fn beat_profiles_from(frames: &[[f64; BANDS]], sample_rate: u32, beat_secs: &[f64]) -> Vec<[f64; BANDS]> {
    if frames.is_empty() {
        return Vec::new();
    }
    let frame_rate = f64::from(sample_rate) / HOP as f64;
    let mut out = Vec::with_capacity(beat_secs.len());
    for (i, &start) in beat_secs.iter().enumerate() {
        // The beat spans up to the next one; the last spans one beat's
        // worth, or to the end.
        let end = beat_secs
            .get(i + 1)
            .copied()
            .unwrap_or_else(|| start + beat_secs.get(1).map_or(0.5, |t| t - beat_secs[0]));
        let first = (start * frame_rate).floor().max(0.0) as usize;
        let last = ((end * frame_rate).ceil() as usize).min(frames.len());
        let mut profile = [0.0_f64; BANDS];
        if first >= last {
            out.push(profile);
            continue;
        }
        for frame in &frames[first..last] {
            for b in 0..BANDS {
                profile[b] += frame[b];
            }
        }
        for slot in &mut profile {
            *slot /= (last - first) as f64;
        }
        out.push(profile);
    }
    out
}

/// Log band energies per frame, over the whole track.
pub fn band_frames(samples: &[f32], sample_rate: u32) -> Vec<[f64; BANDS]> {
    if samples.len() < FRAME {
        return Vec::new();
    }
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FRAME);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let window: Vec<f32> = (0..FRAME)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::PI * 2.0 * i as f32 / FRAME as f32).cos())
        .collect();
    // Which band each bin belongs to, or none.
    let hz_per_bin = f64::from(sample_rate) / FRAME as f64;
    let band_of: Vec<Option<usize>> = (0..output.len())
        .map(|bin| {
            let hz = bin as f64 * hz_per_bin;
            BAND_EDGES.windows(2).position(|edge| hz >= edge[0] && hz < edge[1])
        })
        .collect();

    let count = (samples.len() - FRAME) / HOP + 1;
    let mut frames = Vec::with_capacity(count);
    for f in 0..count {
        let start = f * HOP;
        let Some(chunk) = samples.get(start..start + FRAME) else { break };
        for (i, slot) in input.iter_mut().enumerate() {
            *slot = chunk.get(i).copied().unwrap_or(0.0) * window.get(i).copied().unwrap_or(0.0);
        }
        if fft.process(&mut input, &mut output).is_err() {
            break;
        }
        let mut energy = [0.0_f64; BANDS];
        for (bin, value) in output.iter().enumerate() {
            if let Some(Some(b)) = band_of.get(bin) {
                energy[*b] += f64::from(value.norm_sqr());
            }
        }
        // Log energy, floored well below anything audible so silence does
        // not become minus infinity.
        let mut profile = [0.0_f64; BANDS];
        for b in 0..BANDS {
            profile[b] = (energy[b] + 1e-6).ln();
        }
        frames.push(profile);
    }
    frames
}
