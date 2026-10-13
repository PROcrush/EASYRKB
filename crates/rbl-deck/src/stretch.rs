//! Playing a track faster or slower without moving its pitch.
//!
//! Behind a trait, deliberately, because the good ones disagree about
//! licensing — Rubber Band R3 is GPL-or-commercial, signalsmith-stretch is
//! MIT, Bungee is MPL-2.0 — and a choice made behind `Stretcher` costs a file
//! to change rather than a rewrite.
//!
//! **The choice was Rubber Band, and with it the GPL**; see `LICENSING.md` and
//! [`crate::rubberband`]. What is here is still the default in an
//! MIT build (`--no-default-features`) and is what MASTER TEMPO falls back to
//! if Rubber Band will not allocate: a WSOLA backend written for this crate,
//! enough to measure the CPU cost of two decks stretching at once and enough
//! to hear. It costs about a fortieth of what Rubber Band does
//! (`--example stretchcost`), which is the trade in the other direction.
//!
//! WSOLA is overlap-add with the overlap put where the waveform agrees with
//! itself. Cutting a segment out at a fixed hop and crossfading it against the
//! last one puts two pieces of a periodic signal together at whatever phase
//! they happen to be at, and on anything tonal that is a warble. Searching a
//! few milliseconds either side for the offset that best matches what came
//! before costs one correlation a hop and removes most of it.
//!
//! **What this backend is not good enough for, measured.** Shifting a key —
//! resampling by an interval and stretching back by its inverse — needs the
//! stretch to hold a pitch to a fraction of a percent, and this holds it to a
//! few percent: asked for two semitones down, the tone came back a quarter of
//! a semitone out, and at one semitone half of one. The arrangement is right
//! and the accuracy is not, so nothing in the interface offers a key shift.
//! It was the clearest argument for the trait, and the argument was accepted:
//! Rubber Band went behind it and lands the same intervals inside 1.4 cents,
//! so the semitone controls have something accurate to sit on. This backend
//! still declines to offer them — `shifts_pitch` is false — because a key
//! shift a quarter of a semitone out is worse than none.
//!
//! The reason is the search's tolerance, which has to stay under the
//! difference between the two hops — a search that reaches the point where the
//! signal matches itself lands there, and the stretch stops happening. Near
//! the file's own speed that leaves too little room to align a phase. The
//! usual answer is a different parameterisation, with the hop several times
//! the overlap so the two are far apart; that is a rewrite of this file, and
//! the right moment for it is when a better stretcher is not going behind the
//! trait after all.

use crate::smooth::Smoothed;

/// Frames in one overlap-add segment: about 23 ms at 44.1 kHz.
///
/// Long enough to hold a cycle of anything above 45 Hz, which is what the
/// similarity search needs to find a match on bass, and short enough that a
/// transient is smeared across one segment rather than several.
const FRAME: usize = 1024;

/// Frames of output each segment advances by: half a frame, so every output
/// sample is the sum of exactly two windowed segments and the Hann windows
/// add to one.
const HOP: usize = FRAME / 2;

/// How far either side of the ideal input position the search looks, in
/// frames — about 5.8 ms, which covers a cycle of anything above 172 Hz and
/// most of one below it.
const SEARCH: usize = 256;

/// Frames the input buffer holds.
///
/// A segment, the search either side of it, and four hops of slack. The slack
/// is what double speed needs: the ideal position runs a hop ahead of the
/// window that has been compacted away, so the buffer has to hold a segment
/// beyond it as well as the segment being used.
const CAPACITY: usize = FRAME + SEARCH * 2 + HOP * 4;

/// The slowest and fastest a deck may be asked to play.
///
/// A CDJ's tempo fader is ±16% by default and ±100% at its widest, so half to
/// double covers everything a deck offers and then some. Outside it the
/// search window stops meaning anything.
pub const MIN_RATIO: f32 = 0.5;
pub const MAX_RATIO: f32 = 2.0;

/// Playing at a different speed without moving the pitch.
///
/// Fed interleaved stereo at the device rate, pulled for interleaved stereo at
/// the device rate. `ratio` is how much input time one unit of output time
/// covers: 1.0 is the file's own speed, 1.06 is six percent fast.
pub trait Stretcher: Send {
    /// The speed. Clamped rather than refused: a fader at its end is a fader
    /// at its end.
    fn set_ratio(&mut self, ratio: f32);

    fn ratio(&self) -> f32;

    /// How many input frames it can take right now.
    fn wants(&self) -> usize;

    /// Feeds interleaved stereo. Returns the frames actually taken, which is
    /// `wants()` at most — the caller keeps the rest.
    fn feed(&mut self, input: &[f32]) -> usize;

    /// Whether `pull` can fill a whole buffer of `frames` without running dry.
    fn ready(&self, frames: usize) -> bool;

    /// Fills `out` with stretched audio, returning the frames written. Fewer
    /// than asked for means it ran out of input.
    fn pull(&mut self, out: &mut [f32]) -> usize;

    /// Drops everything held. A seek is not a continuation.
    fn reset(&mut self);

    /// Whether this backend can move the pitch without moving the tempo.
    ///
    /// False by default, because most cannot: the WSOLA backend here holds a
    /// pitch to a few percent, and a key shift a quarter of a semitone out is
    /// worse than no key shift. The interface offers the semitone buttons and
    /// KEY SYNC only where this is true.
    fn shifts_pitch(&self) -> bool {
        false
    }

    /// The frequency multiplier: 1.0 is the file's own pitch, and a semitone
    /// up is `2f32.powf(1.0 / 12.0)`. Ignored where `shifts_pitch` is false.
    fn set_pitch_scale(&mut self, _scale: f32) {}

    fn pitch_scale(&self) -> f32 {
        1.0
    }
}

/// Overlap-add with a similarity search: see the module comment.
pub struct Wsola {
    /// Interleaved stereo, oldest first, at most `CAPACITY` frames.
    input: Vec<f32>,
    /// Frames held in `input`.
    held: usize,
    /// Where the next segment ideally starts, in frames from the front of
    /// `input`. Fractional, because a ratio is.
    ///
    /// The *ideal*, not where the last one was taken from. The search moves a
    /// segment by a few milliseconds to make it join cleanly, and if the next
    /// position were measured from there that deviation would be carried
    /// forward and added to by the next: on a steady tone every hop lands on
    /// the same phase and the deviation is the same sign every time, which
    /// measured as a 20% error in how long the output was. Anchoring on the
    /// nominal position keeps the deviation to the one segment it belongs to.
    at: f32,
    /// The segment that would naturally have followed the last one used, kept
    /// to search against. Interleaved, `HOP` frames.
    template: Vec<f32>,
    /// Output waiting to be pulled: the second half of the last segment,
    /// windowed, ready to be added to the first half of the next.
    tail: Vec<f32>,
    /// Fully overlapped output, interleaved, waiting to go out.
    ready: Vec<f32>,
    /// How much of `ready` has been handed over.
    taken: usize,
    window: Vec<f32>,
    /// How far the search may wander from the ideal position, in frames.
    ///
    /// Not a constant. The template is a verbatim slice of the input a
    /// synthesis hop ahead of the last segment, so if the search can reach
    /// that point it finds the signal matching itself perfectly and lands
    /// there — which is the position that plays at the file's own speed, and
    /// the stretch quietly stops happening. Kept to half the difference
    /// between the two hops, which puts that trap outside the range at every
    /// ratio and closes the search to nothing at ratio 1, where there is no
    /// discontinuity to hide anyway.
    tolerance: usize,
    ratio: Smoothed,
    target: f32,
    /// Whether anything has been through it yet, so the first segment is not
    /// crossfaded against silence.
    started: bool,
}

impl Wsola {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        // A Hann window over a whole segment. Two of them a hop apart sum to
        // exactly one, which is what makes the overlap transparent.
        let window = (0..FRAME)
            .map(|i| {
                #[allow(clippy::cast_precision_loss, reason = "1024 fits a float exactly")]
                let phase = i as f32 / FRAME as f32;
                0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
            })
            .collect();
        Self {
            input: vec![0.0; CAPACITY * 2],
            held: 0,
            at: 0.0,
            template: vec![0.0; HOP * 2],
            tail: vec![0.0; HOP * 2],
            ready: vec![0.0; HOP * 2],
            taken: HOP,
            window,
            tolerance: 0,
            ratio: Smoothed::new(1.0, sample_rate),
            target: 1.0,
            started: false,
        }
    }

    /// The frames of input one output hop consumes at the current speed.
    fn hop_in(&mut self) -> f32 {
        // Smoothed, and per hop rather than per frame: a tempo fader moved
        // mid-segment must not tear the segment it is in the middle of.
        let mut ratio = 1.0;
        for _ in 0..HOP {
            ratio = self.ratio.step(self.target);
        }
        #[allow(clippy::cast_precision_loss, reason = "a hop is 512")]
        let hop = HOP as f32;
        hop * ratio
    }

    /// How well the `HOP` frames at `offset` match the template, as a plain
    /// dot product on the channel sum.
    ///
    /// A dot product rather than a normalised correlation: the segments being
    /// compared are a few milliseconds apart in the same recording, so their
    /// levels are the same to within nothing, and normalising costs two more
    /// passes for a decision that does not change.
    fn score(&self, offset: usize) -> f32 {
        let mut sum = 0.0;
        for i in 0..HOP {
            let a = self.input.get(((offset + i) * 2)..((offset + i) * 2 + 2));
            let b = self.template.get((i * 2)..(i * 2 + 2));
            if let (Some(a), Some(b)) = (a, b) {
                sum += (a[0] + a[1]) * (b[0] + b[1]);
            }
        }
        sum
    }

    /// The offset in `input`, near `self.at`, that best continues the last
    /// segment.
    fn best_offset(&self) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss,
                reason = "`at` is clamped into the buffer before this is called")]
        let ideal = self.at as usize;
        if !self.started {
            return ideal;
        }
        let from = ideal.saturating_sub(self.tolerance);
        let last = self.held.saturating_sub(FRAME);
        let to = (ideal + self.tolerance).min(last);
        let mut best = ideal.min(last);
        let mut score = f32::NEG_INFINITY;
        let mut offset = from;
        while offset <= to {
            let now = self.score(offset);
            if now > score {
                score = now;
                best = offset;
            }
            offset += 1;
        }
        best
    }

    /// Whether there is input for another segment.
    fn can_advance(&self) -> bool {
        // A whole segment from where the next one starts, plus the search
        // window beyond it.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss,
                reason = "`at` is non-negative and bounded by the buffer")]
        let needed = self.at as usize + FRAME + SEARCH;
        self.held >= needed.min(CAPACITY) && self.held >= FRAME
    }

    /// Produces one hop of output, if there is input for it.
    fn advance(&mut self) -> bool {
        if !self.can_advance() {
            return false;
        }
        // The hop first: the search's tolerance depends on it.
        let hop_in = self.hop_in();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss,
                reason = "a hop difference is at most one hop")]
        let spread = ((HOP as f32 - hop_in).abs() * 0.5) as usize;
        self.tolerance = spread.min(SEARCH);
        let offset = self.best_offset();

        // Window the segment and overlap-add: its first half onto the tail the
        // last one left, its second half kept as the next tail.
        for i in 0..HOP {
            let w_head = self.window.get(i).copied().unwrap_or(0.0);
            let w_tail = self.window.get(i + HOP).copied().unwrap_or(0.0);
            for channel in 0..2 {
                let head = self
                    .input
                    .get((offset + i) * 2 + channel)
                    .copied()
                    .unwrap_or(0.0);
                let tail = self
                    .input
                    .get((offset + i + HOP) * 2 + channel)
                    .copied()
                    .unwrap_or(0.0);
                if let Some(slot) = self.ready.get_mut(i * 2 + channel) {
                    let previous = self.tail.get(i * 2 + channel).copied().unwrap_or(0.0);
                    // The very first segment has no tail to sit on, so it goes
                    // out whole rather than fading up out of nothing.
                    *slot = if self.started { previous + head * w_head } else { head };
                }
                if let Some(slot) = self.tail.get_mut(i * 2 + channel) {
                    *slot = tail * w_tail;
                }
            }
        }

        // What would naturally have come next, to search against on the way in.
        for i in 0..HOP {
            for channel in 0..2 {
                let value = self
                    .input
                    .get((offset + i + HOP) * 2 + channel)
                    .copied()
                    .unwrap_or(0.0);
                if let Some(slot) = self.template.get_mut(i * 2 + channel) {
                    *slot = value;
                }
            }
        }

        self.started = true;
        self.taken = 0;
        self.at += hop_in;

        // Drop what is behind the search window, so the buffer never grows.
        // Behind the *earlier* of the ideal and where this segment was taken
        // from, since the next search reaches back from the ideal.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss,
                reason = "`at` is non-negative and bounded by the buffer")]
        let keep_from = (self.at as usize).min(offset).saturating_sub(SEARCH);
        if keep_from > 0 {
            let frames = self.held - keep_from;
            self.input.copy_within(keep_from * 2..self.held * 2, 0);
            self.held = frames;
            #[allow(clippy::cast_precision_loss, reason = "an offset inside a 2k buffer")]
            let moved = keep_from as f32;
            self.at -= moved;
        }
        true
    }
}

impl Stretcher for Wsola {
    fn set_ratio(&mut self, ratio: f32) {
        self.target = if ratio.is_finite() { ratio.clamp(MIN_RATIO, MAX_RATIO) } else { 1.0 };
    }

    fn ratio(&self) -> f32 {
        self.target
    }

    fn wants(&self) -> usize {
        CAPACITY - self.held
    }

    fn feed(&mut self, input: &[f32]) -> usize {
        let frames = (input.len() / 2).min(self.wants());
        let Some(from) = input.get(..frames * 2) else { return 0 };
        let Some(into) = self.input.get_mut(self.held * 2..(self.held + frames) * 2) else {
            return 0;
        };
        into.copy_from_slice(from);
        self.held += frames;
        frames
    }

    fn ready(&self, frames: usize) -> bool {
        // What is already overlapped, plus the one hop another segment would
        // add. Asked exactly the way `advance` decides, rather than estimated
        // from how much input is held: an estimate that ignored where in the
        // buffer the next segment starts said yes while `advance` said no, and
        // the deck starved with a caller that believed it was full.
        let waiting = HOP.saturating_sub(self.taken);
        waiting + if self.can_advance() { HOP } else { 0 } >= frames
    }

    fn pull(&mut self, out: &mut [f32]) -> usize {
        let frames = out.len() / 2;
        let mut done = 0;
        while done < frames {
            if self.taken >= HOP && !self.advance() {
                break;
            }
            let take = (HOP - self.taken).min(frames - done);
            let from = self.ready.get(self.taken * 2..(self.taken + take) * 2);
            let into = out.get_mut(done * 2..(done + take) * 2);
            if let (Some(from), Some(into)) = (from, into) {
                into.copy_from_slice(from);
            }
            self.taken += take;
            done += take;
        }
        done
    }

    fn reset(&mut self) {
        self.held = 0;
        self.at = 0.0;
        self.taken = HOP;
        self.started = false;
        self.tail.fill(0.0);
        self.template.fill(0.0);
        self.ready.fill(0.0);
    }
}

/// Playing at a different speed the way a record does: the pitch moves with
/// it.
///
/// The other half of a tempo control. With Master Tempo off, a deck pitched up
/// is pitched up — that is what a turntable does and what a CDJ does with the
/// key lock off, and it is not a defect to be corrected. Reading the input at
/// a fractional rate with linear interpolation is the whole of it.
pub struct Varispeed {
    input: Vec<f32>,
    held: usize,
    /// Where the next output sample is read from, in frames.
    at: f32,
    ratio: Smoothed,
    target: f32,
}

impl Varispeed {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            input: vec![0.0; CAPACITY * 2],
            held: 0,
            at: 0.0,
            ratio: Smoothed::new(1.0, sample_rate),
            target: 1.0,
        }
    }

    /// One channel of the frame at `at`, between the two samples either side.
    fn sample(&self, at: f32, channel: usize) -> f32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss,
                reason = "`at` is non-negative and bounded by the buffer")]
        let whole = at as usize;
        let fraction = at - at.floor();
        let a = self.input.get(whole * 2 + channel).copied().unwrap_or(0.0);
        let b = self.input.get((whole + 1) * 2 + channel).copied().unwrap_or(a);
        a + (b - a) * fraction
    }
}

impl Stretcher for Varispeed {
    fn set_ratio(&mut self, ratio: f32) {
        self.target = if ratio.is_finite() { ratio.clamp(MIN_RATIO, MAX_RATIO) } else { 1.0 };
    }

    fn ratio(&self) -> f32 {
        self.target
    }

    fn wants(&self) -> usize {
        CAPACITY - self.held
    }

    fn feed(&mut self, input: &[f32]) -> usize {
        let frames = (input.len() / 2).min(self.wants());
        let Some(from) = input.get(..frames * 2) else { return 0 };
        let Some(into) = self.input.get_mut(self.held * 2..(self.held + frames) * 2) else {
            return 0;
        };
        into.copy_from_slice(from);
        self.held += frames;
        frames
    }

    fn ready(&self, frames: usize) -> bool {
        #[allow(clippy::cast_precision_loss, reason = "a frame count inside a 2k buffer")]
        let left = self.held as f32 - self.at - 1.0;
        left >= frames as f32 * self.target
    }

    fn pull(&mut self, out: &mut [f32]) -> usize {
        let mut done = 0;
        for frame in out.chunks_exact_mut(2) {
            // One sample short of the end: interpolation needs the frame after.
            #[allow(clippy::cast_precision_loss, reason = "a frame count inside a 2k buffer")]
            let last = self.held as f32 - 1.0;
            if self.at >= last {
                break;
            }
            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = self.sample(self.at, channel);
            }
            self.at += self.ratio.step(self.target);
            done += 1;
        }
        // Drop what has been read, so the buffer never grows.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss,
                reason = "`at` is non-negative and bounded by the buffer")]
        let used = self.at as usize;
        if used > 0 {
            self.input.copy_within(used * 2..self.held * 2, 0);
            self.held -= used;
            #[allow(clippy::cast_precision_loss, reason = "a frame count inside a 2k buffer")]
            let moved = used as f32;
            self.at -= moved;
        }
        done
    }

    fn reset(&mut self) {
        self.held = 0;
        self.at = 0.0;
    }
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    fn sine(hz: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let t = i as f32 / RATE as f32;
                let value = (t * hz * std::f32::consts::TAU).sin();
                [value, value]
            })
            .collect()
    }

    /// Runs `input` through at `ratio` and returns everything that came out.
    fn stretch(input: &[f32], ratio: f32) -> Vec<f32> {
        let mut wsola = Wsola::new(RATE);
        wsola.set_ratio(ratio);
        let mut out = Vec::new();
        let mut block = vec![0.0_f32; 512 * 2];
        let mut fed = 0;
        loop {
            while wsola.wants() > 0 && fed < input.len() / 2 {
                let take = wsola.wants().min(input.len() / 2 - fed);
                let Some(slice) = input.get(fed * 2..(fed + take) * 2) else { break };
                fed += wsola.feed(slice);
            }
            let produced = wsola.pull(&mut block);
            if produced == 0 {
                break;
            }
            out.extend_from_slice(block.get(..produced * 2).unwrap_or(&[]));
        }
        out
    }

    /// How often the left channel crosses zero going up, which is the tone's
    /// frequency however the timing was stretched.
    fn crossings(out: &[f32]) -> usize {
        let left: Vec<f32> = out.chunks_exact(2).map(|f| f[0]).collect();
        left.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
    }

    #[test]
    fn playing_at_its_own_speed_gives_back_what_went_in() {
        // Ratio 1 has to be transparent, or every deck not using the tempo
        // fader pays for one that is.
        let input = sine(440.0, RATE as usize / 2);
        let out = stretch(&input, 1.0);
        let frames = out.len() / 2;
        assert!(frames > RATE as usize / 4, "only {frames} frames came out");
        // Past the first segment, where there is nothing to overlap against.
        let from = FRAME * 2;
        for (i, frame) in out.chunks_exact(2).enumerate().skip(from).take(2_000) {
            let want = input.get(i * 2).copied().unwrap_or(0.0);
            assert!(
                (frame[0] - want).abs() < 0.05,
                "frame {i} came back as {} rather than {want}",
                frame[0],
            );
        }
    }

    #[test]
    fn slower_is_longer_and_faster_is_shorter() {
        let input = sine(220.0, RATE as usize);
        let half = stretch(&input, 0.5).len() / 2;
        let same = stretch(&input, 1.0).len() / 2;
        let double = stretch(&input, 2.0).len() / 2;
        // Half speed is about twice as long, double speed about half. Within a
        // few percent: the ends are ragged by up to a segment either way.
        assert!(
            (half as f32 / same as f32 - 2.0).abs() < 0.1,
            "half speed gave {half} against {same}",
        );
        assert!(
            (double as f32 / same as f32 - 0.5).abs() < 0.1,
            "double speed gave {double} against {same}",
        );
    }

    #[test]
    fn the_pitch_does_not_move_with_the_speed() {
        // The whole point. A resampler would give 220 Hz at half speed and
        // 880 at double; this has to give 440 at both.
        let input = sine(440.0, RATE as usize);
        for ratio in [0.5_f32, 0.75, 1.0, 1.5, 2.0] {
            let out = stretch(&input, ratio);
            let frames = out.len() / 2;
            let hz = crossings(&out) as f32 * RATE as f32 / frames as f32;
            assert!(
                (hz - 440.0).abs() < 12.0,
                "at {ratio}x the tone came out at {hz} Hz, not 440",
            );
        }
    }

    /// The same harness, for whichever backend.
    fn through(mut deck: impl Stretcher, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        let mut block = vec![0.0_f32; 512 * 2];
        let mut fed = 0;
        loop {
            while deck.wants() > 0 && fed < input.len() / 2 {
                let take = deck.wants().min(input.len() / 2 - fed);
                let Some(slice) = input.get(fed * 2..(fed + take) * 2) else { break };
                fed += deck.feed(slice);
            }
            let produced = deck.pull(&mut block);
            if produced == 0 {
                break;
            }
            out.extend_from_slice(block.get(..produced * 2).unwrap_or(&[]));
        }
        out
    }

    #[test]
    fn without_the_key_lock_the_pitch_moves_with_the_speed() {
        // The other half of a tempo control, and not a defect: a record
        // pitched up is pitched up.
        let input = sine(440.0, RATE as usize);
        for ratio in [0.5_f32, 0.75, 1.0, 1.5, 2.0] {
            let mut deck = Varispeed::new(RATE);
            deck.set_ratio(ratio);
            let out = through(deck, &input);
            let frames = out.len() / 2;
            assert!(frames > 1_000, "at {ratio}x only {frames} frames came out");
            let hz = crossings(&out) as f32 * RATE as f32 / frames as f32;
            let want = 440.0 * ratio;
            assert!(
                (hz - want).abs() < want * 0.03,
                "at {ratio}x the tone came out at {hz} Hz, not {want}",
            );
        }
    }

    #[test]
    fn varispeed_at_its_own_speed_gives_back_what_went_in() {
        let input = sine(440.0, RATE as usize / 4);
        let mut deck = Varispeed::new(RATE);
        deck.set_ratio(1.0);
        let out = through(deck, &input);
        for (i, frame) in out.chunks_exact(2).enumerate().skip(2_000).take(2_000) {
            let want = input.get(i * 2).copied().unwrap_or(0.0);
            assert!(
                (frame[0] - want).abs() < 0.01,
                "frame {i} came back as {} rather than {want}",
                frame[0],
            );
        }
    }

    #[test]
    fn a_speed_outside_what_a_deck_offers_is_clamped_rather_than_refused() {
        let mut wsola = Wsola::new(RATE);
        wsola.set_ratio(9.0);
        assert!((wsola.ratio() - MAX_RATIO).abs() < f32::EPSILON);
        wsola.set_ratio(0.0);
        assert!((wsola.ratio() - MIN_RATIO).abs() < f32::EPSILON);
        wsola.set_ratio(f32::NAN);
        assert!((wsola.ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_reset_leaves_nothing_of_the_last_track_behind() {
        let mut wsola = Wsola::new(RATE);
        let input = sine(440.0, FRAME * 4);
        wsola.feed(&input);
        let mut out = vec![0.0_f32; 512 * 2];
        assert!(wsola.pull(&mut out) > 0);
        wsola.reset();
        // Nothing on hand, so nothing comes out until it is fed again.
        assert_eq!(wsola.pull(&mut out), 0);
        assert_eq!(wsola.wants(), CAPACITY);
    }

    #[test]
    fn it_never_takes_more_than_it_can_hold() {
        let mut wsola = Wsola::new(RATE);
        let input = sine(100.0, CAPACITY * 4);
        let taken = wsola.feed(&input);
        assert_eq!(taken, CAPACITY);
        assert_eq!(wsola.feed(&input), 0, "a full buffer must take nothing");
    }
}

