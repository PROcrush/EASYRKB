//! What the licence bought: a stretcher that holds a pitch.
//!
//! The WSOLA backend beside it holds one to a few percent — asked for two
//! semitones down the tone came back a quarter of a semitone out, which is why
//! nothing in the interface offered a key shift. These are the same
//! measurements against Rubber Band R3. It lands every interval from an octave
//! down to an octave up within **1.4 cents** — a hundredth of a semitone —
//! against WSOLA's 25 to 50. The gate is set at 3 cents rather than at what was
//! measured, because the single-file build uses Apple's vDSP FFT on macOS and
//! its own everywhere else, and the two need not agree to the last cent.
#![cfg(feature = "rubberband")]
#![allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rbl_deck::{RubberBand, Stretcher};

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

/// Frequency from rising zero crossings, over the settled middle only: the
/// first and last moments of any stretcher are its window filling and
/// emptying, and neither is the steady state being measured.
fn hz_of(out: &[f32]) -> f32 {
    let frames = out.len() / 2;
    let from = frames / 4;
    let to = frames - frames / 4;
    let left: Vec<f32> = out[from * 2..to * 2].chunks_exact(2).map(|f| f[0]).collect();
    let crossings = left.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    crossings as f32 * RATE as f32 / (to - from) as f32
}

fn through(deck: &mut impl Stretcher, input: &[f32]) -> Vec<f32> {
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
        out.extend_from_slice(&block[..produced * 2]);
    }
    out
}

#[test]
fn a_semitone_is_a_semitone() {
    // The failure this replaces: two semitones down came back a quarter of a
    // semitone out, and one semitone half of one.
    for semitones in [-12.0_f32, -2.0, -1.0, 1.0, 2.0, 12.0] {
        let scale = 2.0_f32.powf(semitones / 12.0);
        let mut deck = RubberBand::new(RATE).expect("the library allocates");
        deck.set_pitch_scale(scale);
        let out = through(&mut deck, &sine(440.0, RATE as usize * 2));
        assert!(out.len() / 2 > RATE as usize, "only {} frames came out", out.len() / 2);

        let want = 440.0 * scale;
        let hz = hz_of(&out);
        let cents = 1200.0 * (hz / want).log2();
        assert!(
            cents.abs() < 3.0,
            "asked for {semitones} semitones: wanted {want:.1} Hz, got {hz:.1}, \
             {cents:.1} cents out",
        );
    }
}

#[test]
fn shifting_the_key_does_not_move_the_tempo() {
    // The other half of a key shift, and the half the resample-and-stretch-back
    // arrangement kept getting right while the pitch went wrong.
    let input = sine(440.0, RATE as usize * 2);
    let plain = {
        let mut deck = RubberBand::new(RATE).expect("the library allocates");
        through(&mut deck, &input).len() / 2
    };
    let mut deck = RubberBand::new(RATE).expect("the library allocates");
    deck.set_pitch_scale(2.0_f32.powf(3.0 / 12.0));
    let shifted = through(&mut deck, &input).len() / 2;

    let drift = (shifted as f32 - plain as f32).abs() / plain as f32;
    assert!(drift < 0.02, "three semitones up changed the length by {:.1}%", drift * 100.0);
}

#[test]
fn the_tempo_still_moves_when_it_is_asked_to() {
    for ratio in [0.5_f32, 0.94, 1.0, 1.06, 2.0] {
        let input = sine(440.0, RATE as usize * 2);
        let mut deck = RubberBand::new(RATE).expect("the library allocates");
        deck.set_ratio(ratio);
        let out = through(&mut deck, &input);
        let frames = out.len() / 2;

        // A ratio of 1.06 covers six percent more input per unit of output, so
        // the output is that much shorter.
        let want = (input.len() / 2) as f32 / ratio;
        let drift = (frames as f32 - want).abs() / want;
        assert!(drift < 0.05, "at {ratio}x got {frames} frames, wanted about {want:.0}");

        // And the pitch stays where it was, which is the whole point.
        let hz = hz_of(&out);
        assert!((hz - 440.0).abs() < 8.0, "at {ratio}x the tone came out at {hz:.1} Hz");
    }
}

#[test]
fn the_pitch_shifting_backend_says_so_and_the_others_do_not() {
    // What the interface reads to decide whether to draw the semitone buttons.
    assert!(RubberBand::new(RATE).expect("the library allocates").shifts_pitch());
    assert!(!rbl_deck::Wsola::new(RATE).shifts_pitch());
}

#[test]
fn a_reset_leaves_nothing_of_the_last_track_behind() {
    let mut deck = RubberBand::new(RATE).expect("the library allocates");
    let input = sine(440.0, RATE as usize / 2);
    through(&mut deck, &input);
    deck.reset();
    let mut out = vec![0.0_f32; 512 * 2];
    assert_eq!(deck.pull(&mut out), 0, "a reset deck must have nothing to give");
}
