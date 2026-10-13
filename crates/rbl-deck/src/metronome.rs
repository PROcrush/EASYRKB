//! The metronome: a click on every beat of a deck's grid while it plays.
//!
//! rekordbox's, from Preferences › Audio: three click sounds and three
//! volumes, switched on per deck from the grid-editing row. The grid is the
//! track's own — the beats the analysis wrote, handed over in output frames
//! — so the click lands where the grid says the beat is, and a wrong grid is
//! heard as a wrong grid, which is the point of a metronome in a grid editor.
//!
//! Added after the channel strip and before the master level: an EQ cut on
//! the deck must not muffle the click, and the master turns it down with
//! everything else. The click is synthesised — a short decaying sine, an
//! octave up on the downbeat — so nothing is read from disk in the callback.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// Which click. rekordbox's Click Sound 01 to 03; higher is brighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickSound {
    One = 1,
    Two = 2,
    Three = 3,
}

impl ClickSound {
    /// The click's pitch on an ordinary beat, in hertz; the downbeat is an
    /// octave up. Spaced so the three are told apart at once.
    fn hz(self) -> f32 {
        match self {
            ClickSound::One => 880.0,
            ClickSound::Two => 1320.0,
            ClickSound::Three => 1760.0,
        }
    }

    /// How long the click rings, in seconds: the brighter, the shorter.
    fn decay(self) -> f32 {
        match self {
            ClickSound::One => 0.035,
            ClickSound::Two => 0.025,
            ClickSound::Three => 0.018,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => ClickSound::One,
            3 => ClickSound::Three,
            _ => ClickSound::Two,
        }
    }
}

/// Small, Middle, Large: the click's peak, as a fraction of full scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickVolume {
    Small,
    Middle,
    Large,
}

impl ClickVolume {
    fn gain(self) -> f32 {
        match self {
            ClickVolume::Small => 0.2,
            ClickVolume::Middle => 0.4,
            ClickVolume::Large => 0.7,
        }
    }
}

/// The metronome as the interface sets it: shared by both decks, read by
/// the callback without a lock.
#[derive(Debug)]
pub struct MetronomeSettings {
    sound: AtomicU8,
    gain: AtomicU32,
}

impl Default for MetronomeSettings {
    fn default() -> Self {
        // The capture's defaults: Click Sound 02, Large [OBS].
        Self { sound: AtomicU8::new(2), gain: AtomicU32::new(ClickVolume::Large.gain().to_bits()) }
    }
}

impl MetronomeSettings {
    pub fn set_sound(&self, sound: ClickSound) {
        self.sound.store(sound as u8, Ordering::Relaxed);
    }

    pub fn sound(&self) -> ClickSound {
        ClickSound::from_u8(self.sound.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, volume: ClickVolume) {
        self.gain.store(volume.gain().to_bits(), Ordering::Relaxed);
    }

    fn gain(&self) -> f32 {
        f32::from_bits(self.gain.load(Ordering::Relaxed))
    }
}

/// One beat of a deck's grid, in output frames at 100 % tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridBeat {
    pub frame: u64,
    /// The first beat of its bar, which clicks an octave up.
    pub downbeat: bool,
}

/// A click in progress: a sine at `hz` decaying over `decay` seconds.
#[derive(Debug, Clone, Copy)]
struct Click {
    hz: f32,
    peak: f32,
    decay: f32,
    /// Frames rendered so far.
    at: u32,
}

/// One deck's metronome.
#[derive(Debug)]
pub struct Metronome {
    on: AtomicBool,
    /// The grid, sorted by frame. Replaced whole on a load; the callback
    /// takes it with `try_lock` and skips a callback rather than wait.
    grid: Mutex<Arc<[GridBeat]>>,
    settings: Arc<MetronomeSettings>,
}

impl Metronome {
    pub fn new(settings: Arc<MetronomeSettings>) -> Self {
        Self { on: AtomicBool::new(false), grid: Mutex::new(Arc::from(Vec::new())), settings }
    }

    pub fn set_on(&self, on: bool) {
        self.on.store(on, Ordering::Relaxed);
    }

    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }

    /// The deck's grid, in output frames, in any order.
    pub fn set_grid(&self, mut beats: Vec<GridBeat>) {
        beats.sort_by_key(|b| b.frame);
        if let Ok(mut grid) = self.grid.lock() {
            *grid = Arc::from(beats);
        }
    }

    /// The beats the playhead crossed moving from `from` to `to` frames of
    /// the track, as offsets into a buffer of `frames` output frames, with
    /// whether each is a downbeat. Nothing when the deck did not move, or
    /// the metronome is off.
    fn crossed(&self, from: u64, to: u64, frames: usize) -> Vec<(usize, bool)> {
        let mut out = Vec::new();
        if !self.is_on() || to <= from || frames == 0 {
            return out;
        }
        let Ok(grid) = self.grid.try_lock() else { return out };
        let first = grid.partition_point(|b| b.frame < from);
        // The first frame after a load is the downbeat, and a beat exactly
        // on `from` is counted here, not by the callback before this one.
        #[allow(clippy::cast_precision_loss, reason = "a buffer is at most a few thousand frames")]
        let scale = frames as f64 / (to - from) as f64;
        for beat in grid.iter().skip(first).take_while(|b| b.frame < to) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "under `frames` by construction")]
            let offset = (((beat.frame - from) as f64) * scale) as usize;
            out.push((offset.min(frames - 1), beat.downbeat));
        }
        out
    }
}

/// The clicks sounding on one deck, carried between callbacks.
#[derive(Debug, Default)]
pub struct MetronomeVoice {
    ringing: Vec<Click>,
}

impl MetronomeVoice {
    /// Adds the deck's clicks for this callback into `out` (interleaved
    /// stereo, `frames` frames), for a playhead that went from `from` to
    /// `to`. The deck's own position is what places a click, so a beat at
    /// half tempo still lands on its beat.
    pub fn render(
        &mut self,
        metronome: &Metronome,
        from: u64,
        to: u64,
        out: &mut [f32],
        rate: u32,
    ) {
        let frames = out.len() / 2;
        let settings = &metronome.settings;
        let sound = settings.sound();
        let peak = settings.gain();
        let starts = metronome.crossed(from, to, frames);
        if starts.is_empty() && self.ringing.is_empty() {
            return;
        }
        #[allow(clippy::cast_precision_loss, reason = "a sample rate is far below 2^24")]
        let rate = rate as f32;
        // Every ringing click continues; every new one starts at its offset.
        let mut pending = starts.into_iter().peekable();
        for (i, frame) in out.chunks_exact_mut(2).enumerate() {
            while pending.peek().is_some_and(|(at, _)| *at <= i) {
                let (_, downbeat) = pending.next().unwrap_or((0, false));
                let hz = if downbeat { sound.hz() * 2.0 } else { sound.hz() };
                self.ringing.push(Click { hz, peak, decay: sound.decay(), at: 0 });
            }
            let mut sum = 0.0_f32;
            for click in &mut self.ringing {
                #[allow(clippy::cast_precision_loss, reason = "a click is a few thousand frames")]
                let t = click.at as f32 / rate;
                // Exponential decay, 60 dB down at `decay` seconds.
                let envelope = (-6.9 * t / click.decay).exp();
                sum += click.peak * envelope * (std::f32::consts::TAU * click.hz * t).sin();
                click.at = click.at.saturating_add(1);
            }
            frame[0] += sum;
            frame[1] += sum;
        }
        // A click 60 dB down is done.
        self.ringing.retain(|c| {
            #[allow(clippy::cast_precision_loss, reason = "a click is a few thousand frames")]
            let t = c.at as f32 / rate;
            t < c.decay
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
mod tests {
    use super::*;

    fn metronome() -> Metronome {
        let m = Metronome::new(Arc::new(MetronomeSettings::default()));
        m.set_grid(vec![
            GridBeat { frame: 1000, downbeat: true },
            GridBeat { frame: 500, downbeat: false },
            GridBeat { frame: 1500, downbeat: false },
        ]);
        m.set_on(true);
        m
    }

    #[test]
    fn the_beats_crossed_are_placed_where_the_playhead_passed_them() {
        let m = metronome();
        // From 400 to 1200 in 800 frames: beats at 500 and 1000, 100 and 600 in.
        assert_eq!(m.crossed(400, 1200, 800), vec![(100, false), (600, true)]);
        // Half speed: 400 to 800 over 800 frames, the beat at 500 lands 200 in.
        assert_eq!(m.crossed(400, 800, 800), vec![(200, false)]);
        // A beat on the boundary belongs to the callback that starts on it.
        assert_eq!(m.crossed(500, 1000, 500), vec![(0, false)]);
        assert_eq!(m.crossed(1000, 1500, 500), vec![(0, true)]);
    }

    #[test]
    fn off_or_still_is_silent() {
        let m = metronome();
        assert!(m.crossed(400, 400, 512).is_empty(), "no movement");
        m.set_on(false);
        assert!(m.crossed(0, 2000, 512).is_empty(), "off");
    }

    #[test]
    fn a_click_starts_at_its_beat_and_dies_away() {
        let m = metronome();
        let mut voice = MetronomeVoice::default();
        let mut out = vec![0.0_f32; 800 * 2];
        voice.render(&m, 400, 1200, &mut out, 44_100);
        // Nothing before the first beat, something at it, both channels alike.
        assert!(out[..200].iter().all(|s| *s == 0.0));
        let loud = out[200..400].iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(loud > 0.1, "the click is heard: {loud}");
        assert_eq!(out[202], out[203]);
        // The downbeat, an octave up, starts at 600 and rings past the end.
        assert!(!voice.ringing.is_empty());
        // And a later callback with no beats lets it die.
        let mut tail = vec![0.0_f32; 4096 * 2];
        voice.render(&m, 1200, 1300, &mut tail, 44_100);
        assert!(voice.ringing.is_empty(), "60 dB down within the callback");
    }

    #[test]
    fn the_volumes_and_sounds_are_the_capture_s_three_each() {
        let s = MetronomeSettings::default();
        assert_eq!(s.sound(), ClickSound::Two);
        assert_eq!(s.gain(), ClickVolume::Large.gain());
        s.set_sound(ClickSound::Three);
        s.set_volume(ClickVolume::Small);
        assert_eq!(s.sound(), ClickSound::Three);
        assert_eq!(s.gain(), 0.2);
    }
}
