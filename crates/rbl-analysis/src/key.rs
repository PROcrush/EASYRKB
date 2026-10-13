//! Musical key by chromagram plus profile matching.
//!
//! Spectral energy is folded onto the twelve pitch classes, then correlated
//! against major and minor profiles. The best of the 24 rotations is the key.
//!
//! The front end is what decides the result, and two things about it were
//! measured to matter more than any profile:
//!
//! - **Energy, not counts.** An earlier version added `ln(1 + |X|)` for every
//!   bin near a semitone, which made a pitch class's total mostly the number
//!   of bins that happen to round to it — a fixed pattern that read most of
//!   the golden playlist as C minor. A bin contributes its magnitude, and
//!   only in proportion to how close it sits to a semitone.
//! - **Harmonics fold down.** A note at f also puts energy at 2f, 3f, 4f…,
//!   which land on other pitch classes (the fifth, the major third) and pull
//!   the answer a fifth away. Each bin also credits the classes of f/2, f/3,
//!   f/4 with decaying weight, so a note's harmonics vote for the note.
//!
//! The profile match is then handed to a **rule pipeline** ([`Rule`]): named
//! corrections, each with its own knobs, applied in order to the ranked
//! candidates. The rules are the ones set out in `rules.md` — a toss-up
//! goes to the minor; when the match is close, the bass in the intro, the
//! outro and on the downbeats names the tonic — and the golden rig reports,
//! per rule, how often it fired, what it fixed and what it broke, so a new
//! rule is judged the same way as a new profile.
//!
//! Every front-end choice is a field of [`KeyOptions`] so the golden rig
//! can search them; the defaults are what scored best on the golden
//! playlist.

use realfft::RealFftPlanner;

/// Chromagram window.
///
/// This must be much longer than the onset window: at 44.1 kHz a 1024-sample
/// FFT resolves 43 Hz, while a semitone at 220 Hz spans about 13 Hz — too
/// coarse to tell pitch classes apart at all in the bass. 8192 gives ~5.4 Hz,
/// enough down to about 90 Hz.
const KEY_FRAME: usize = 8192;
/// Frames advance by half a window.
const KEY_HOP: usize = 4096;

/// Names matching `djmdKey.ScaleName`, so a detected key can be interned
/// against rekordbox's own table.
const MAJOR_NAMES: [&str; 12] =
    ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const MINOR_NAMES: [&str; 12] =
    ["Cm", "Dbm", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "Abm", "Am", "Bbm", "Bm"];

/// A pair of key profiles: how much each scale degree belongs to a major
/// key and to a minor one, from the tonic up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Profile {
    pub major: [f64; 12],
    pub minor: [f64; 12],
}

impl Profile {
    /// Krumhansl & Kessler's probe-tone ratings.
    pub const KRUMHANSL: Self = Self {
        major: [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88],
        minor: [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17],
    };
    /// Temperley's revision of them.
    pub const TEMPERLEY: Self = Self {
        major: [5.0, 2.0, 3.5, 2.0, 4.5, 4.0, 2.0, 4.5, 2.0, 3.5, 1.5, 4.0],
        minor: [5.0, 2.0, 3.5, 4.5, 2.0, 4.0, 2.0, 4.5, 3.5, 2.0, 1.5, 4.0],
    };
    /// Shaath's, tuned on popular music.
    pub const SHAATH: Self = Self {
        major: [6.6, 2.0, 3.5, 2.3, 4.6, 4.0, 2.5, 5.2, 2.4, 3.7, 2.3, 3.4],
        minor: [6.5, 2.7, 3.5, 5.4, 2.6, 3.5, 2.5, 5.2, 4.0, 2.7, 4.3, 3.2],
    };
    /// Faraldo's, fitted to electronic dance music. Values as Essentia
    /// ships them (`key.cpp`, profile `edma`); the correlation is
    /// scale-free, so only the shape matters.
    pub const EDMA: Self = Self {
        major: [1.00, 0.29, 0.50, 0.40, 0.60, 0.56, 0.32, 0.80, 0.31, 0.45, 0.42, 0.39],
        minor: [1.00, 0.31, 0.44, 0.58, 0.33, 0.49, 0.29, 0.78, 0.43, 0.29, 0.53, 0.32],
    };
    /// Faraldo's "assume minor" variant: a flat major profile, so a track
    /// reads as major only when nothing fits, and an `edma`-like minor.
    pub const EDMM: Self = Self {
        major: [0.083; 12],
        minor: [
            0.172_353_48, 0.04, 0.076_100_9, 0.12, 0.056_214_98, 0.085_278_53, 0.049_791_5,
            0.134_510_01, 0.074_589_16, 0.050_030_23, 0.091_878_79, 0.055_451_06,
        ],
    };
    /// Faraldo's, fitted on Beatport's catalogue and gated: scale degrees
    /// outside the key are zero. Essentia's default for his method.
    pub const BGATE: Self = Self {
        major: [1.00, 0.00, 0.42, 0.00, 0.53, 0.37, 0.00, 0.77, 0.00, 0.38, 0.21, 0.30],
        minor: [1.00, 0.00, 0.36, 0.39, 0.00, 0.38, 0.00, 0.74, 0.27, 0.00, 0.42, 0.23],
    };
    /// The same Beatport fit before gating.
    pub const BRAW: Self = Self {
        major: [1.0000, 0.1573, 0.4200, 0.1570, 0.5296, 0.3669, 0.1632, 0.7711, 0.1676, 0.3827, 0.2113, 0.2965],
        minor: [1.0000, 0.2330, 0.3615, 0.3905, 0.2925, 0.3777, 0.1961, 0.7425, 0.2701, 0.2161, 0.4228, 0.2272],
    };
    /// The notes of the key and nothing else: tonic and fifth carry it, the
    /// rest of the scale supports.
    pub const DIATONIC: Self = Self {
        major: [3.0, 0.0, 1.0, 0.0, 2.0, 1.0, 0.0, 2.5, 0.0, 1.0, 0.0, 1.0],
        minor: [3.0, 0.0, 1.0, 2.0, 0.0, 1.0, 0.0, 2.5, 1.0, 0.0, 1.0, 0.5],
    };
}

/// Which front end turns audio into a pitch-class profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrontEnd {
    /// Every spectrum bin credits its pitch and its subharmonics'
    /// ([`chroma_frames`]); the front end this crate started with.
    Chroma,
    /// Ángel Faraldo's edmkey method as Essentia's `KeyExtractor` runs it:
    /// spectral peaks, whitening, an HPCP with harmonic folding, the
    /// average gated at 0.2 of its peak ([`edmkey_profile`]).
    Edmkey(EdmkeyOptions),
}

/// Everything the front end and the matcher are tuned by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyOptions {
    pub front_end: FrontEnd,
    pub profile: Profile,
    /// Lowest and highest frequency a bin may have to count.
    pub low_hz: f64,
    pub high_hz: f64,
    /// Exponent applied to a bin's magnitude: 1 sums magnitudes, 2 energy,
    /// and a fraction compresses loud partials.
    pub power: f64,
    /// How many subharmonics a bin credits (1 is just its own pitch class),
    /// and how much each further one counts relative to the one before.
    pub harmonics: usize,
    pub harmonic_decay: f64,
    /// Whether each frame's chroma is scaled to a peak of 1 before it is
    /// added, so a loud drop does not outweigh a quiet intro.
    pub frame_norm: bool,
    /// Whether to find the track's tuning first. A track a third of a
    /// semitone off concert pitch puts every note between two classes.
    pub tuning: bool,
}

/// Sub-bins per semitone in a frame's chroma: enough to see a track sitting
/// between semitones.
pub const SUBBINS: usize = 3;
/// Sub-bins per octave.
pub const BINS: usize = 12 * SUBBINS;

/// The defaults are the best of 3,600 combinations measured on the golden
/// playlist (`golden key`): 130 of 155 exact, 138 within a fifth or the
/// relative key. Of the 25 misses, 11 are the same tonic in the other mode
/// — mostly tracks rekordbox calls major — 8 are a fifth away and 6 are
/// elsewhere. Each of these was tried and did not move the count: a mode
/// decision from the third above the tonic, a separate smaller bias for
/// the mode, the tuning estimate, a bass-only chroma added in, per-frame
/// normalisation, and profiles learned from the playlist itself with each
/// track held out (132 at best, within noise of the shipped 130).
impl Default for KeyOptions {
    fn default() -> Self {
        Self {
            front_end: FrontEnd::Edmkey(EdmkeyOptions::DEFAULT),
            // Faraldo's own profile. On the golden playlist edma at a bias
            // of 0.1 and Shaath at 0.2 tie at 140 of 155; Krumhansl gives
            // 139 and bgate, Essentia's default, 138.
            profile: Profile::EDMA,
            low_hz: 55.0,
            // Above 2 kHz there is little but hats and the upper partials of
            // everything; 1 kHz, 1.5 kHz, 3 kHz and 5 kHz all scored lower.
            high_hz: 2000.0,
            power: 1.0,
            harmonics: 4,
            harmonic_decay: 0.6,
            frame_norm: false,
            // Measured to change nothing on the golden playlist, which is
            // mastered at concert pitch throughout; kept for material that
            // is not.
            tuning: false,
        }
    }
}

/// A correction applied to the ranked candidates after the profile match.
///
/// Each is one of the rules in `rules.md`, with the knobs it needs. They are
/// applied in the order given; each sees the ranking the previous one left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rule {
    /// A toss-up between a major key and its parallel minor goes to the
    /// minor: `bias` is added to every minor key's score. 89 % of the
    /// library is minor, and a correlation alone cannot know that.
    PreferMinor { bias: f64 },
    /// When the match is close, the bass names the tonic. Fires when the
    /// best key's score exceeds the best score at any *other* tonic by less
    /// than `margin`; then the bass's strongest pitch class over `source`
    /// becomes the tonic, and the mode is whichever scores higher there.
    BassRoot { margin: f64, source: BassSource },
    /// The bass votes on every key: `weight` times the bass's chroma at a
    /// key's tonic (scaled to a peak of 1) is added to that key's score,
    /// so a clear bass root tips a close match without overriding a clear
    /// one.
    BassVote { weight: f64, source: BassSource },
}

/// Where the bass is read for [`Rule::BassRoot`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BassSource {
    /// The first and last `secs` of the track: in intros and outros the
    /// bass is usually playing the root.
    Edges { secs: f64 },
    /// The first frame of every bar.
    Downbeats,
    /// Both of the above.
    EdgesAndDownbeats { secs: f64 },
    /// The frames between beats, where the kick's own pitch is absent.
    OffBeats,
    /// The second eighth of every beat: the kick, tail included, takes the
    /// first sixteenth to eighth of a beat, so the second eighth is the
    /// bass line alone.
    SecondEighth,
    /// The second eighths of the first `bars` bars after each phrase
    /// start: where a bass line states the root before it moves.
    PhraseStart { bars: usize },
    /// Every frame.
    Whole,
}

/// The grid the bass rules read against.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyGrid {
    /// Every beat's time in seconds and its number in the bar (1 is the
    /// downbeat).
    pub beats: Vec<(f64, u16)>,
    /// Where phrases start, in seconds.
    pub phrase_starts: Vec<f64>,
}

/// The rules that ship, in order.
pub const DEFAULT_RULES: &[Rule] = &[Rule::PreferMinor { bias: 0.1 }];

/// What the rules are allowed to look at.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvidence {
    /// The whole track's chroma.
    pub chroma: [f64; 12],
    /// The bass band's chroma over each [`BassSource`], in the order the
    /// enum lists them: edges, downbeats, both, off-beats, second eighth,
    /// phrase starts, whole.
    pub bass: [[f64; 12]; 7],
}

/// The profile match's scores, one per key: `scores[tonic][minor as usize]`.
pub type Scores = [[f64; 2]; 12];

/// One key's standing after a rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    pub tonic: usize,
    pub minor: bool,
}

/// What a rule did to a track, for the golden rig.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Applied {
    pub rule: Rule,
    pub before: Verdict,
    pub after: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicalKey {
    /// e.g. `Ebm`.
    pub name: String,
    /// 0 = C.
    pub tonic: u8,
    pub minor: bool,
}

impl MusicalKey {
    /// Camelot code, e.g. `2A`.
    pub fn camelot(&self) -> String {
        // The wheel is the circle of fifths: 1A is Abm and 1B is B, so each
        // family is anchored on its own tonic (Ab = 8, B = 11).
        let anchor = if self.minor { 8 } else { 11 };
        let index = (i32::from(self.tonic) - anchor).rem_euclid(12);
        let number = (index * 7).rem_euclid(12) + 1;
        format!("{number}{}", if self.minor { 'A' } else { 'B' })
    }
}

/// Detects the key. Returns `None` when the audio is too short or has no
/// discernible pitch content, rather than guessing C major.
pub fn detect_key(samples: &[f32], sample_rate: u32) -> Option<MusicalKey> {
    detect_key_with(samples, sample_rate, KeyOptions::default(), DEFAULT_RULES, &KeyGrid::default()).map(|r| r.key)
}

/// A detection and how the rules arrived at it.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyReport {
    pub key: MusicalKey,
    /// What the profile match alone said.
    pub matched: Verdict,
    /// Every rule that changed the verdict, in order.
    pub applied: Vec<Applied>,
}

/// Detects the key with the front end and the rules under the caller's
/// control. `grid` is the beat grid, for the rules that read the bass on
/// or between beats; empty when no grid is known.
#[allow(clippy::needless_pass_by_value, reason = "a Copy options struct")]
pub fn detect_key_with(
    samples: &[f32],
    sample_rate: u32,
    options: KeyOptions,
    rules: &[Rule],
    grid: &KeyGrid,
) -> Option<KeyReport> {
    let evidence = gather_evidence(samples, sample_rate, options, rules, grid)?;
    judge(&evidence, options, rules)
}

/// Whether any of `rules` reads the bass on the grid, so that a caller
/// without a grid yet knows whether to wait for one.
pub fn wants_grid(rules: &[Rule]) -> bool {
    rules.iter().any(|r| matches!(r, Rule::BassRoot { .. } | Rule::BassVote { .. }))
}

/// Everything the profile match and the rules will look at. The bass is
/// only read when a rule asks for it.
pub fn gather_evidence(
    samples: &[f32],
    sample_rate: u32,
    options: KeyOptions,
    rules: &[Rule],
    grid: &KeyGrid,
) -> Option<KeyEvidence> {
    let (chroma, offset) = match options.front_end {
        FrontEnd::Chroma => {
            let frames = chroma_frames(samples, sample_rate, options)?;
            let offset = if options.tuning { tuning_offset(&frames) } else { 0 };
            (fold_frames(&frames, options.frame_norm, offset), offset)
        }
        FrontEnd::Edmkey(edmkey) => (edmkey_profile(samples, sample_rate, edmkey)?, 0),
    };
    let wants_bass = rules.iter().find_map(|r| match r {
        Rule::BassRoot { source, .. } | Rule::BassVote { source, .. } => Some(*source),
        Rule::PreferMinor { .. } => None,
    });
    let bass = match wants_bass {
        Some(source) => bass_evidence(samples, sample_rate, options, offset, source, grid),
        None => [[0.0; 12]; 7],
    };
    Some(KeyEvidence { chroma, bass })
}

/// The band the bass root is read from, and how many subharmonics fold
/// into it. Measured on the golden playlist against every other choice:
/// the bass's strongest class names rekordbox's tonic on 102 of 155 tracks
/// with this band read between beats, against 45 with 40–120 Hz.
pub const BASS_LOW_HZ: f64 = 80.0;
pub const BASS_HIGH_HZ: f64 = 400.0;
pub const BASS_HARMONICS: usize = 2;

/// The bass band's chroma over every [`BassSource`], in the enum's order.
/// Only the requested source's edge length and bar count are honoured; the
/// others use 45 s and four bars.
pub fn bass_evidence(
    samples: &[f32],
    sample_rate: u32,
    options: KeyOptions,
    offset: i32,
    source: BassSource,
    grid: &KeyGrid,
) -> [[f64; 12]; 7] {
    let beats = &grid.beats;
    let bass = KeyOptions { low_hz: BASS_LOW_HZ, high_hz: BASS_HIGH_HZ, harmonics: BASS_HARMONICS, ..options };
    let Some(frames) = chroma_frames(samples, sample_rate, bass) else {
        return [[0.0; 12]; 7];
    };
    let hop_secs = KEY_HOP as f64 / f64::from(sample_rate);
    let total_secs = frames.len() as f64 * hop_secs;
    let edge_secs = match source {
        BassSource::Edges { secs } | BassSource::EdgesAndDownbeats { secs } => secs,
        _ => 45.0,
    };
    let phrase_bars = match source {
        BassSource::PhraseStart { bars } => bars,
        _ => 4,
    };
    // A bar's length from the grid, for the phrase windows.
    let bar_secs = beats.windows(2).map(|w| w[1].0 - w[0].0).next().unwrap_or(0.5) * 4.0;
    let in_phrase_window = |t: f64| {
        grid.phrase_starts.iter().any(|&start| t >= start && t < start + phrase_bars as f64 * bar_secs)
    };
    let frame_of = |t: f64| (t / hop_secs).floor().max(0.0) as usize;
    let on_beat: std::collections::HashSet<usize> = beats.iter().map(|&(t, _)| frame_of(t)).collect();
    let on_downbeat: std::collections::HashSet<usize> =
        beats.iter().filter(|&&(_, n)| n == 1).map(|&(t, _)| frame_of(t)).collect();
    let at_edge = |i: usize| {
        let t = i as f64 * hop_secs;
        t < edge_secs || t >= total_secs - edge_secs
    };
    // Frames whose centre falls in the second half of a beat. Beats are in
    // order, so one pass over both.
    let centre_secs = |i: usize| i as f64 * hop_secs + KEY_FRAME as f64 / 2.0 / f64::from(sample_rate);
    let mut second_eighth = vec![false; frames.len()];
    let mut b = 0usize;
    for (i, slot) in second_eighth.iter_mut().enumerate() {
        let t = centre_secs(i);
        while b + 1 < beats.len() && beats[b + 1].0 <= t {
            b += 1;
        }
        if let (Some(&(start, _)), Some(&(next, _))) = (beats.get(b), beats.get(b + 1)) {
            *slot = t >= start.midpoint(next) && t < next;
        }
    }
    let pick = |keep: &dyn Fn(usize) -> bool| -> [f64; 12] {
        let kept: Vec<[f64; BINS]> = frames.iter().enumerate().filter(|(i, _)| keep(*i)).map(|(_, f)| *f).collect();
        fold_frames(&kept, options.frame_norm, offset)
    };
    [
        pick(&at_edge),
        pick(&|i| on_downbeat.contains(&i)),
        pick(&|i| at_edge(i) || on_downbeat.contains(&i)),
        pick(&|i| !on_beat.contains(&i)),
        pick(&|i| second_eighth[i]),
        pick(&|i| second_eighth[i] && in_phrase_window(centre_secs(i))),
        pick(&|_| true),
    ]
}

/// The profile match followed by the rules.
pub fn judge(evidence: &KeyEvidence, options: KeyOptions, rules: &[Rule]) -> Option<KeyReport> {
    if total(&evidence.chroma) <= f64::EPSILON {
        return None;
    }
    let mut scores = match_profiles(&evidence.chroma, options.profile);
    let matched = best_of(&scores);
    let mut verdict = matched;
    let mut applied = Vec::new();
    for &rule in rules {
        let next = apply(rule, evidence, &mut scores, verdict);
        if next != verdict {
            applied.push(Applied { rule, before: verdict, after: next });
            verdict = next;
        }
    }
    let names = if verdict.minor { MINOR_NAMES } else { MAJOR_NAMES };
    let key = MusicalKey {
        name: (*names.get(verdict.tonic)?).to_owned(),
        tonic: u8::try_from(verdict.tonic).ok()?,
        minor: verdict.minor,
    };
    Some(KeyReport { key, matched, applied })
}

/// Pearson correlation of the chroma against every rotation of both
/// profiles.
pub fn match_profiles(chroma: &[f64; 12], profile: Profile) -> Scores {
    let mut scores = [[0.0_f64; 2]; 12];
    for (tonic, slot) in scores.iter_mut().enumerate() {
        slot[0] = correlate(chroma, &profile.major, tonic);
        slot[1] = correlate(chroma, &profile.minor, tonic);
    }
    scores
}

/// The best-scoring key.
fn best_of(scores: &Scores) -> Verdict {
    let mut best = (f64::NEG_INFINITY, Verdict { tonic: 0, minor: false });
    for (tonic, slot) in scores.iter().enumerate() {
        for (m, &score) in slot.iter().enumerate() {
            if score > best.0 {
                best = (score, Verdict { tonic, minor: m == 1 });
            }
        }
    }
    best.1
}

/// The bass chroma a source names.
fn bass_of(evidence: &KeyEvidence, source: BassSource) -> [f64; 12] {
    evidence.bass[match source {
        BassSource::Edges { .. } => 0,
        BassSource::Downbeats => 1,
        BassSource::EdgesAndDownbeats { .. } => 2,
        BassSource::OffBeats => 3,
        BassSource::SecondEighth => 4,
        BassSource::PhraseStart { .. } => 5,
        BassSource::Whole => 6,
    }]
}

/// Applies one rule to the ranking, returning the verdict it leaves.
fn apply(rule: Rule, evidence: &KeyEvidence, scores: &mut Scores, current: Verdict) -> Verdict {
    match rule {
        Rule::BassVote { weight, source } => {
            let bass = bass_of(evidence, source);
            let peak = bass.iter().fold(0.0_f64, |a, &b| a.max(b));
            if peak <= 0.0 {
                return current;
            }
            for (tonic, slot) in scores.iter_mut().enumerate() {
                let vote = weight * bass[tonic] / peak;
                slot[0] += vote;
                slot[1] += vote;
            }
            best_of(scores)
        }
        Rule::PreferMinor { bias } => {
            for slot in scores.iter_mut() {
                slot[1] += bias;
            }
            best_of(scores)
        }
        Rule::BassRoot { margin, source } => {
            let own = scores[current.tonic][usize::from(current.minor)];
            let rival = scores
                .iter()
                .enumerate()
                .filter(|(tonic, _)| *tonic != current.tonic)
                .flat_map(|(_, slot)| slot.iter().copied())
                .fold(f64::NEG_INFINITY, f64::max);
            if own - rival >= margin {
                return current;
            }
            let bass = bass_of(evidence, source);
            if total(&bass) <= f64::EPSILON {
                return current;
            }
            let tonic = bass
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map_or(current.tonic, |(i, _)| i);
            Verdict { tonic, minor: scores[tonic][1] >= scores[tonic][0] }
        }
    }
}

fn total(chroma: &[f64; 12]) -> f64 {
    chroma.iter().sum()
}

/// Which sub-bin the track's notes centre on: the tuning, as a signed
/// number of sub-bins from concert pitch.
///
/// Whichever of the three sub-bins per semitone collects the most energy
/// across the whole track is where the notes are.
pub fn tuning_offset(frames: &[[f64; BINS]]) -> i32 {
    let mut by_offset = [0.0_f64; SUBBINS];
    for frame in frames {
        for (i, value) in frame.iter().enumerate() {
            by_offset[i % SUBBINS] += value;
        }
    }
    let best = by_offset
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i);
    // Sub-bin 0 is the semitone's centre, 1 a third sharp, 2 a third flat.
    if best == SUBBINS - 1 { -1 } else { best as i32 }
}

/// How much a class's neighbouring sub-bins count: a third of a semitone
/// off the centre is the skirt of a note more often than a note, so it
/// counts a quarter.
const NEIGHBOUR_WEIGHT: f64 = 0.25;

/// Sums per-frame chroma into twelve classes, each frame scaled to a peak
/// of 1 first when asked. `offset` is the tuning from [`tuning_offset`]:
/// the sub-bin at the centre of each class, with its two neighbours at
/// [`NEIGHBOUR_WEIGHT`].
pub fn fold_frames(frames: &[[f64; BINS]], frame_norm: bool, offset: i32) -> [f64; 12] {
    let mut out = [0.0_f64; 12];
    for frame in frames {
        let mut folded = [0.0_f64; 12];
        for (class, slot) in folded.iter_mut().enumerate() {
            let centre = (class * SUBBINS) as i32 + offset;
            let at = |i: i32| frame[(i.rem_euclid(BINS as i32)) as usize];
            *slot = at(centre) + NEIGHBOUR_WEIGHT * (at(centre - 1) + at(centre + 1));
        }
        let peak = folded.iter().fold(0.0_f64, |a, &b| a.max(b));
        if peak <= 0.0 {
            continue;
        }
        let scale = if frame_norm { 1.0 / peak } else { 1.0 };
        for (slot, value) in out.iter_mut().zip(folded.iter()) {
            *slot += value * scale;
        }
    }
    out
}

/// Pearson correlation between the chroma and a profile rotated to `tonic`.
fn correlate(chroma: &[f64; 12], profile: &[f64; 12], tonic: usize) -> f64 {
    let rotated: Vec<f64> = (0..12)
        .map(|i| profile.get((i + 12 - tonic) % 12).copied().unwrap_or(0.0))
        .collect();
    let mean_c: f64 = chroma.iter().sum::<f64>() / 12.0;
    let mean_p: f64 = rotated.iter().sum::<f64>() / 12.0;
    let mut num = 0.0;
    let mut den_c = 0.0;
    let mut den_p = 0.0;
    for i in 0..12 {
        let dc = chroma.get(i).copied().unwrap_or(0.0) - mean_c;
        let dp = rotated.get(i).copied().unwrap_or(0.0) - mean_p;
        num += dc * dp;
        den_c += dc * dc;
        den_p += dp * dp;
    }
    if den_c <= 0.0 || den_p <= 0.0 {
        return 0.0;
    }
    num / (den_c.sqrt() * den_p.sqrt())
}

/// One chroma per frame at `SUBBINS` per semitone, un-normalised. `None`
/// when the audio is too short.
pub fn chroma_frames(samples: &[f32], sample_rate: u32, options: KeyOptions) -> Option<Vec<[f64; BINS]>> {
    if samples.len() < KEY_FRAME * 2 || sample_rate == 0 {
        return None;
    }
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(KEY_FRAME);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();

    let window: Vec<f32> = (0..KEY_FRAME)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::PI * 2.0 * i as f32 / KEY_FRAME as f32).cos())
        .collect();

    // What each bin contributes, worked out once: the sub-bins it credits
    // (its own pitch and its subharmonics') and the weight of each. A bin
    // between two sub-bins is split between them in proportion, so nothing
    // is dropped for falling between the cracks: down at 110 Hz a sub-bin
    // is under half an FFT bin wide.
    let hz_per_bin = f64::from(sample_rate) / KEY_FRAME as f64;
    let mut credits: Vec<Vec<(usize, f64)>> = Vec::with_capacity(output.len());
    for bin in 0..output.len() {
        let freq = bin as f64 * hz_per_bin;
        let mut list = Vec::new();
        if bin > 0 && freq >= options.low_hz && freq <= options.high_hz {
            let mut weight = 1.0;
            for h in 1..=options.harmonics.max(1) {
                let midi = 69.0 + 12.0 * (freq / h as f64 / 440.0).log2();
                let sub = midi * SUBBINS as f64;
                let below = sub.floor();
                let frac = sub - below;
                let index = |x: f64| (x as i64).rem_euclid(BINS as i64) as usize;
                list.push((index(below), weight * (1.0 - frac)));
                list.push((index(below + 1.0), weight * frac));
                weight *= options.harmonic_decay;
            }
        }
        credits.push(list);
    }

    let mut frames = Vec::with_capacity(samples.len() / KEY_HOP);
    let mut start = 0;
    while start + KEY_FRAME <= samples.len() {
        let Some(chunk) = samples.get(start..start + KEY_FRAME) else { break };
        for (i, slot) in input.iter_mut().enumerate() {
            *slot = chunk.get(i).copied().unwrap_or(0.0) * window.get(i).copied().unwrap_or(0.0);
        }
        if fft.process(&mut input, &mut output).is_err() {
            break;
        }
        let mut chroma = [0.0_f64; BINS];
        for (bin, value) in output.iter().enumerate() {
            let Some(list) = credits.get(bin) else { continue };
            if list.is_empty() {
                continue;
            }
            let magnitude = f64::from(value.norm()).powf(options.power);
            for &(index, weight) in list {
                chroma[index] += magnitude * weight;
            }
        }
        frames.push(chroma);
        start += KEY_HOP;
    }
    Some(frames)
}

// ---------------------------------------------------------------- edmkey

/// Faraldo's front end, with Essentia's `KeyExtractor` defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdmkeyOptions {
    /// Frame and hop, in samples: 4096 and 4096, no overlap.
    pub frame: usize,
    pub hop: usize,
    /// Peaks are taken between these frequencies.
    pub min_hz: f64,
    pub max_hz: f64,
    /// The strongest this many peaks per frame count.
    pub max_peaks: usize,
    /// A peak below this magnitude is ignored.
    pub peak_threshold: f64,
    /// Whether peaks are whitened against the spectral envelope.
    pub whitening: bool,
    /// Subharmonics folded into each peak's contribution: 4 in Essentia,
    /// which with its weighting table makes the fifth below and the major
    /// third below count as well as the peak's own class.
    pub harmonics: usize,
    /// Bins under this share of the averaged profile's peak are zeroed.
    pub gate: f64,
    /// Whether whitening also tilts peaks down by 5 dB per kHz, as
    /// Essentia's does.
    pub tilt: bool,
}

impl EdmkeyOptions {
    /// Essentia's `KeyExtractor` defaults.
    pub const DEFAULT: Self = Self {
        frame: 4096,
        hop: 4096,
        min_hz: 25.0,
        max_hz: 3500.0,
        max_peaks: 60,
        peak_threshold: 0.0001,
        whitening: true,
        harmonics: 4,
        gate: 0.2,
        tilt: true,
    };
}

impl Default for EdmkeyOptions {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The twelve-class profile of a track by Faraldo's method: spectral
/// peaks, whitened, folded into an HPCP per frame, averaged over the
/// track, scaled to a peak of 1 and gated. `None` when the audio is too
/// short.
pub fn edmkey_profile(samples: &[f32], sample_rate: u32, options: EdmkeyOptions) -> Option<[f64; 12]> {
    let frame = options.frame.max(64);
    let hop = options.hop.max(1);
    if samples.len() < frame * 2 || sample_rate == 0 {
        return None;
    }
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(frame);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let window: Vec<f32> = (0..frame)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::PI * 2.0 * i as f32 / frame as f32).cos())
        .collect();
    let hz_per_bin = f64::from(sample_rate) / frame as f64;
    let harmonic_table = harmonic_table(options.harmonics);

    let mut total = [0.0_f64; 12];
    let mut frames = 0usize;
    let mut spectrum = vec![0.0_f64; output.len()];
    let mut start = 0;
    while start + frame <= samples.len() {
        let Some(chunk) = samples.get(start..start + frame) else { break };
        for (i, slot) in input.iter_mut().enumerate() {
            *slot = chunk.get(i).copied().unwrap_or(0.0) * window.get(i).copied().unwrap_or(0.0);
        }
        if fft.process(&mut input, &mut output).is_err() {
            break;
        }
        for (slot, value) in spectrum.iter_mut().zip(output.iter()) {
            *slot = f64::from(value.norm());
        }
        let mut peaks = spectral_peaks(&spectrum, hz_per_bin, options);
        if options.whitening {
            whiten(&spectrum, hz_per_bin, options.max_hz, options.tilt, &mut peaks);
        }
        let mut hpcp = [0.0_f64; 12];
        for &(hz, magnitude) in &peaks {
            if hz < options.min_hz || hz > options.max_hz {
                continue;
            }
            for &(semitones, strength) in &harmonic_table {
                // The hypothesised fundamental this peak may be a harmonic of.
                let f = hz * 2.0_f64.powf(-semitones / 12.0);
                // In semitones from C, so the classes line up with the
                // profiles: A4 is MIDI 69, and 69 is 9 past a C.
                let bin = (f / 440.0).log2() * 12.0 + 69.0;
                // Cosine weighting one semitone wide: the nearest bin, and
                // the one beyond it only when exactly between two.
                let left = (bin - 0.5).ceil() as i64;
                let right = (bin + 0.5).floor() as i64;
                for i in left..=right {
                    let distance = (bin - i as f64).abs();
                    let weight = (std::f64::consts::PI * distance).cos();
                    let class = i.rem_euclid(12) as usize;
                    hpcp[class] += weight * magnitude * magnitude * strength * strength;
                }
            }
        }
        for (slot, value) in total.iter_mut().zip(hpcp.iter()) {
            *slot += value;
        }
        frames += 1;
        start += hop;
    }
    if frames == 0 {
        return None;
    }
    // The average, scaled to a peak of 1 and gated.
    let peak = total.iter().fold(0.0_f64, |a, &b| a.max(b));
    if peak <= 0.0 {
        return None;
    }
    for slot in &mut total {
        *slot /= peak;
        if *slot < options.gate {
            *slot = 0.0;
        }
    }
    Some(total)
}

/// Essentia's HPCP harmonic table: for harmonic `i` (1-based, up to
/// `harmonics + 1`), the semitone offset of its fundamental below the
/// peak, folded into one octave, and its strength. Offsets that fold onto
/// each other add their strengths, so with four harmonics the peak's own
/// class counts 3, the fifth below 1 and the major third below 0.86.
fn harmonic_table(harmonics: usize) -> Vec<(f64, f64)> {
    let mut table: Vec<(f64, f64)> = Vec::new();
    for i in 0..=harmonics {
        let raw = 12.0 * ((i + 1) as f64).log2();
        let octave_weight = (raw / 12.0 * 0.5).max(1.0);
        let mut semitone = raw;
        while semitone >= 12.0 - 1e-5 {
            semitone -= 12.0;
        }
        match table.iter_mut().find(|(s, _)| (*s - semitone).abs() < 1e-5) {
            Some((_, strength)) => *strength += 1.0 / octave_weight,
            None => table.push((semitone, 1.0 / octave_weight)),
        }
    }
    table
}

/// Local maxima of the magnitude spectrum between the option's
/// frequencies, the strongest `max_peaks` of them, each placed between
/// bins by a parabola through its neighbours. `(hz, magnitude)`.
pub fn spectral_peaks(spectrum: &[f64], hz_per_bin: f64, options: EdmkeyOptions) -> Vec<(f64, f64)> {
    let mut peaks: Vec<(f64, f64)> = Vec::new();
    for i in 1..spectrum.len().saturating_sub(1) {
        let (a, b, c) = (spectrum[i - 1], spectrum[i], spectrum[i + 1]);
        if !(b > a && b >= c) || b < options.peak_threshold {
            continue;
        }
        let denom = a - 2.0 * b + c;
        let offset = if denom.abs() > f64::EPSILON { 0.5 * (a - c) / denom } else { 0.0 };
        let hz = (i as f64 + offset.clamp(-0.5, 0.5)) * hz_per_bin;
        let magnitude = b - 0.25 * (a - c) * offset;
        if hz >= options.min_hz && hz <= options.max_hz {
            peaks.push((hz, magnitude));
        }
    }
    peaks.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
    peaks.truncate(options.max_peaks);
    peaks
}

/// The grid the whitening envelope is sampled on, in hertz.
const RESOLUTION: f64 = 100.0;

/// Essentia's spectral whitening: each peak is measured against the
/// spectrum's energy envelope around it (a band from 0.66× to 1.58× its
/// frequency, or ±50 Hz at the bottom) and comes out as its level
/// relative to that envelope, capped at 1, with a tilt of −5 dB per kHz.
/// A loud region and a quiet one then count alike.
pub fn whiten(spectrum: &[f64], hz_per_bin: f64, max_hz: f64, tilt: bool, peaks: &mut [(f64, f64)]) {
    let bins = spectrum.len();
    if bins < 2 {
        return;
    }
    let spectral_range = (bins - 1) as f64 * hz_per_bin;
    // The envelope, in dB, on a 100 Hz grid.
    let mut envelope: Vec<(f64, f64)> = Vec::new();
    let mut freq = 0.0;
    while freq <= max_hz && freq <= spectral_range {
        let bf = freq - (freq * 0.34).max(50.0);
        let ef = freq + (freq * 0.58).max(50.0);
        let first = ((bf / spectral_range * (bins as f64 - 1.0) + 0.5) as i64).clamp(0, bins as i64 - 1) as usize;
        let last = ((ef / spectral_range * (bins as f64 - 1.0) + 0.5) as i64).max(first as i64 + 1).min(bins as i64) as usize;
        let centre = first as f64 / 2.0 + last as f64 / 2.0;
        let half = last as f64 - centre;
        let (mut weighted, mut weights) = (0.0, 0.0);
        for (i, &value) in spectrum.iter().enumerate().take(last).skip(first) {
            let mut w = 1.0 - (i as f64 - centre).abs() / half;
            w *= w;
            w *= w;
            let energy = value * value;
            w *= energy;
            weighted += energy * w;
            weights += w;
        }
        let mean = if weights > 0.0 { weighted / weights } else { 0.0 };
        envelope.push((freq, 20.0 * mean.sqrt().max(1e-12).log10()));
        freq += RESOLUTION;
    }
    if envelope.len() >= 2 {
        let last = envelope.len() - 1;
        envelope[last].1 = envelope[last - 1].1;
    }
    let envelope_at = |hz: f64| -> f64 {
        let i = envelope.partition_point(|(f, _)| *f <= hz);
        match (envelope.get(i.wrapping_sub(1)), envelope.get(i)) {
            (Some(&(f0, v0)), Some(&(f1, v1))) if f1 > f0 => v0 + (v1 - v0) * (hz - f0) / (f1 - f0),
            (Some(&(_, v0)), _) => v0,
            (None, Some(&(_, v1))) => v1,
            _ => 0.0,
        }
    };
    for (hz, magnitude) in peaks.iter_mut() {
        let db = 20.0 * magnitude.max(1e-12).log10();
        // Essentia passes a peak within one grid step of the top through
        // unwhitened; on a spectrum that is not scaled to 1 that peak would
        // outweigh every whitened one, so here it is whitened against the
        // envelope's last value like the rest.
        let env = envelope_at(hz.min(max_hz - RESOLUTION));
        let mut white = if db > env {
            0.0
        } else if db > env - 30.0 {
            db - env
        } else {
            -200.0
        };
        if tilt {
            white -= 20.0 * *hz / 4000.0;
        }
        *magnitude = 10.0_f64.powf(white / 20.0);
    }
}
