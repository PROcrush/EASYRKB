//! How long a jump takes to be heard. READ-ONLY.
//!
//! "Jumping from the start to near the end hesitates" is a claim about the
//! time between asking for a position and hearing it. This measures exactly
//! that: it pulls the sink in real time the way a device callback does, asks
//! for a position, and counts the buffers that come back silent before the
//! audio of the new position arrives.
//!
//! Distance is the variable. A jump forward by a second lands in what the
//! decoder has already read; a jump to the far end of a file is a demuxer seek
//! and a decoder reset, and that is the one that is supposed to hesitate.
//!
//! `cargo run --release -p rbl-deck --example jumpcost -- <audio file>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Frames a device callback asks for at a time: 11.6 ms at 44.1 kHz.
const BUFFER: usize = 512;

/// Long enough that a jump that never arrives is reported rather than waited
/// for. Nothing here should come close to it.
const PATIENCE: Duration = Duration::from_secs(3);

fn main() {
    let Some(path) = std::env::args().nth(1).map(std::path::PathBuf::from) else {
        println!("usage: jumpcost <audio file>");
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
    let Ok(rbl_deck::DeckEvent::Loaded { total_frames, sample_rate, .. }) =
        rx.recv_timeout(Duration::from_secs(20))
    else {
        println!("did not load");
        return;
    };
    let total_ms = total_frames * 1000 / u64::from(sample_rate.max(1));
    println!("loaded: {total_frames} frames at {sample_rate} — {:.1} s", total_ms as f64 / 1000.0);

    let period = Duration::from_secs_f64(BUFFER as f64 / 44_100.0);
    println!("buffer {BUFFER} frames = {:.1} ms\n", period.as_secs_f64() * 1000.0);
    println!("  {:<22} {:>9} {:>8}", "jump", "silence", "buffers");

    // Every jump starts from the same place, so what changes between rows is
    // the distance and nothing else.
    let from_ms = 5_000_u64;
    let targets: Vec<(String, u64)> = vec![
        ("+1 s".to_owned(), from_ms + 1_000),
        ("+10 s".to_owned(), from_ms + 10_000),
        ("+60 s".to_owned(), from_ms + 60_000),
        ("to the last 10 s".to_owned(), total_ms.saturating_sub(10_000)),
        ("back to the start".to_owned(), 0),
    ];

    for (label, to_ms) in targets {
        engine.seek_ms(rbl_deck::Deck::A, from_ms as f64);
        engine.play(rbl_deck::Deck::A);
        // Let the ring fill, so what is measured is the jump rather than the
        // start of playback.
        let settle = Instant::now();
        while settle.elapsed() < Duration::from_millis(400) {
            let _ = sink.pull(BUFFER);
            std::thread::sleep(period);
        }

        // What is being timed is audio *from the new position*, not any audio
        // at all: the ring still holds the old position's blocks when the ask
        // goes in, and they play on for a moment. The playhead is what says
        // which is which — it only advances past the target once the callback
        // has taken a block of the new generation.
        let target = to_ms * u64::from(sample_rate) / 1000;
        let asked = Instant::now();
        engine.seek_ms(rbl_deck::Deck::A, to_ms as f64);
        let mut silent = 0_u32;
        let mut heard = None;
        while asked.elapsed() < PATIENCE {
            let next = Instant::now() + period;
            let out = sink.pull(BUFFER);
            // Landed *and* sounding: within a second of the target, which the
            // playhead only reaches once the callback has taken a block of the
            // new generation. A bare `>` would call a jump back to the start
            // instant, since the playhead is already past zero.
            let at = engine.snapshot().a.position_frames;
            let landed = at >= target && at <= target + u64::from(sample_rate);
            if landed && out.iter().any(|s| s.abs() > 1e-6) {
                heard = Some(asked.elapsed());
                break;
            }
            if out.iter().all(|s| s.abs() <= 1e-6) {
                silent += 1;
            }
            let left = next.saturating_duration_since(Instant::now());
            if !left.is_zero() {
                std::thread::sleep(left);
            }
        }
        engine.pause(rbl_deck::Deck::A);
        std::thread::sleep(Duration::from_millis(120));
        match heard {
            Some(at) => println!(
                "  {label:<22} {:>7.0} ms {silent:>8}",
                at.as_secs_f64() * 1000.0,
            ),
            None => println!("  {label:<22} {:>9} {silent:>8}", "never"),
        }
    }
}
