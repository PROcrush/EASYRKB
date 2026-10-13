//! What a fast drag sounds like: pulls the sink in real time while the pointer
//! is dragged, and reports how long the audio ran after the hand stopped.
//!
//! A drag that grinds on for seconds after the pointer stops is the bug this
//! guards. READ-ONLY.
//!
//! `cargo run --release -p rbl-deck --example scrubcheck -- <audio file>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const BUFFER: usize = 512;

/// A run of silence this long inside a drag is a hole you can hear.
const GAP_FRAMES: usize = 64;

/// How far a step must stand out from its neighbours to count as a click.
///
/// An absolute threshold is useless on music: a loud 5 kHz peak steps by most
/// of full scale between samples all by itself. A click is a step that does
/// not belong to what is around it, so this compares each step with the
/// average step over the surrounding few milliseconds.
const CLICK_RATIO: f32 = 12.0;

/// Samples either side a step is compared against: about two milliseconds.
const CLICK_NEIGHBOURHOOD: usize = 96;

/// Where the stream went quiet or jumped, which is what glitching sounds like.
fn report_continuity(label: &str, stream: &[f32], rate: u32) {
    let frames: Vec<f32> = stream.chunks_exact(2).map(|f| f[0]).collect();
    let mut gaps = 0_u32;
    let mut longest = 0_usize;
    let mut run = 0_usize;
    for sample in &frames {
        if *sample == 0.0 {
            run += 1;
        } else {
            if run >= GAP_FRAMES {
                gaps += 1;
                longest = longest.max(run);
            }
            run = 0;
        }
    }
    if run >= GAP_FRAMES {
        gaps += 1;
        longest = longest.max(run);
    }
    let steps: Vec<f32> = frames.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let mut clicks = 0_u32;
    for i in CLICK_NEIGHBOURHOOD..steps.len().saturating_sub(CLICK_NEIGHBOURHOOD) {
        let around: f32 = steps[i - CLICK_NEIGHBOURHOOD..i]
            .iter()
            .chain(&steps[i + 1..i + 1 + CLICK_NEIGHBOURHOOD])
            .sum::<f32>()
            / (CLICK_NEIGHBOURHOOD * 2) as f32;
        // A silent neighbourhood has nothing to stand out from; the gap count
        // above is what covers that case.
        if around > 1e-4 && steps[i] > around * CLICK_RATIO {
            clicks += 1;
        }
    }
    println!(
        "{label}: {} frames, {gaps} gaps (longest {:.0} ms), {clicks} clicks",
        frames.len(),
        longest as f64 * 1000.0 / f64::from(rate.max(1)),
    );
}

fn main() {
    let Some(path) = std::env::args().nth(1).map(std::path::PathBuf::from) else {
        println!("usage: scrubcheck <audio file>");
        return;
    };
    let (tx, rx) = mpsc::channel();
    let events: rbl_deck::EventSink = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    let (sink_tx, sink_rx) = mpsc::channel();
    let engine = rbl_deck::Engine::with_sink(
        move |render| {
            let sink = Arc::new(rbl_deck::NullSink::new(44_100, render));
            let _ = sink_tx.send(Arc::clone(&sink));
            Ok(sink as Arc<dyn rbl_deck::Sink>)
        },
        &events,
    )
    .expect("engine");
    let sink = sink_rx.recv().expect("sink");
    engine.load(rbl_deck::Deck::A, &path);
    let Ok(rbl_deck::DeckEvent::Loaded { sample_rate, .. }) =
        rx.recv_timeout(Duration::from_secs(20))
    else {
        println!("did not load");
        return;
    };

    let period = Duration::from_secs_f64(BUFFER as f64 / f64::from(sample_rate));
    engine.seek_ms(rbl_deck::Deck::A, 60_000.0);
    std::thread::sleep(Duration::from_millis(200));
    engine.scrub_begin(rbl_deck::Deck::A);

    // A slow drag first. What matters is not whether a buffer had *any*
    // audio but whether the stream was continuous: a gap of a few hundred
    // samples inside an otherwise loud buffer is exactly the click that is
    // heard as glitching, and a per-buffer count cannot see it.
    let slow_start = Instant::now();
    let mut slow_at = 60_000_u64;
    let mut stream: Vec<f32> = Vec::new();
    // Long enough to walk out of the decoded window and force a refill, which
    // is where a drag would starve if the refill outran what is buffered.
    let mut passes = 0_u32;
    while slow_start.elapsed() < Duration::from_millis(4_000) {
        // A hand, not a machine: it moves, hesitates, moves on. The pauses are
        // the point — they are where the head comes to rest, and where a stop
        // that cuts rather than fades is heard as a crack.
        passes += 1;
        if passes % 12 < 8 {
            slow_at += 30;
            engine.scrub_to_ms(rbl_deck::Deck::A, slow_at as f64);
        }
        stream.extend_from_slice(&sink.pull(BUFFER));
        std::thread::sleep(period);
    }
    report_continuity("a slow drag", &stream, sample_rate);
    println!("  dragged {} ms of music over 4 s", slow_at - 60_000);

    // A flick: the pointer crosses thirty seconds of music in a fifth of a
    // second, then stops dead.
    let start = Instant::now();
    let mut at = slow_at;
    let mut sounding_while_dragging = 0_u32;
    while start.elapsed() < Duration::from_millis(200) {
        at += 1_500;
        engine.scrub_to_ms(rbl_deck::Deck::A, at as f64);
        if sink.pull(BUFFER).iter().any(|s| *s != 0.0) {
            sounding_while_dragging += 1;
        }
        std::thread::sleep(period);
    }

    // The hand has stopped. How long does the audio keep going?
    let stopped = Instant::now();
    let mut last_sound = stopped;
    while stopped.elapsed() < Duration::from_secs(3) {
        if sink.pull(BUFFER).iter().any(|s| *s != 0.0) {
            last_sound = Instant::now();
        }
        std::thread::sleep(period);
    }
    engine.scrub_end(rbl_deck::Deck::A);
    std::thread::sleep(Duration::from_millis(100));

    println!("a flick: {} ms of music in 200 ms of wall time", at - slow_at);
    println!("  audible in {sounding_while_dragging} of the drag's buffers");
    println!("  kept sounding for {:?} after the pointer stopped", last_sound - stopped);
    println!(
        "  landed at {} ms, pointer was at {at} ms",
        engine.snapshot().a.position_frames * 1000 / u64::from(sample_rate.max(1)),
    );
}
