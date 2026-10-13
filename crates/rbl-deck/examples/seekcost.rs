//! Can a scrub be audible? Pulls the sink in real time while seeking at a
//! drag's rate, and counts the buffers that came back silent.
//!
//! A gap is a buffer the decode thread had nothing ready for, which is what a
//! stuttering scrub sounds like. READ-ONLY.
//!
//! `cargo run --release -p rbl-deck --example seekcost -- <audio file>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Frames a device callback asks for at a time, and how long that lasts.
const BUFFER: usize = 512;

fn main() {
    let Some(path) = std::env::args().nth(1).map(std::path::PathBuf::from) else {
        println!("usage: seekcost <audio file>");
        return;
    };
    let (tx, rx) = mpsc::channel();
    let events: rbl_deck::EventSink = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    // The engine hands its mixer to the closure; keep the sink built around it
    // so this can pull it in real time, the way a device callback would.
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
    println!("loaded: {total_frames} frames at {sample_rate}");

    let period = Duration::from_secs_f64(BUFFER as f64 / 44_100.0);
    println!("buffer {BUFFER} frames = {:.1} ms", period.as_secs_f64() * 1000.0);
    for hz in [60_u64, 30, 10, 0] {
        engine.play(rbl_deck::Deck::A);
        // Let the ring fill before the drag starts, as it would have been.
        std::thread::sleep(Duration::from_millis(300));
        let gap = 1000_u64.checked_div(hz).map_or(Duration::MAX, Duration::from_millis);
        let start = Instant::now();
        let mut last_seek = Instant::now();
        let mut at_ms = 30_000_u64;
        let mut pulls = 0_u32;
        let mut moved = 0_u32;
        while start.elapsed() < Duration::from_secs(2) {
            let next = Instant::now() + period;
            if hz > 0 && last_seek.elapsed() >= gap {
                at_ms += 40;
                engine.seek_ms(rbl_deck::Deck::A, at_ms as f64);
                last_seek = Instant::now();
            }
            // Pull as a device callback would, and count the buffers that came
            // back as anything but silence — a silent one is a gap you hear.
            let out = sink.pull(BUFFER);
            if out.iter().any(|s| *s != 0.0) {
                moved += 1;
            }
            pulls += 1;
            let left = next.saturating_duration_since(Instant::now());
            if !left.is_zero() {
                std::thread::sleep(left);
            }
        }
        engine.pause(rbl_deck::Deck::A);
        let label = if hz == 0 { "no seeks".to_owned() } else { format!("{hz} seeks/s") };
        println!(
            "  {label:12} {} of {pulls} buffers had audio ({:.0}% gaps)",
            moved,
            100.0 - f64::from(moved) / f64::from(pulls) * 100.0,
        );
    }
}
