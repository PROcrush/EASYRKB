//! Whether a drag on a freshly loaded track tracks the pointer the way a drag
//! after the first play does. The bug this guards: the first drag sounds
//! wrong and sits behind the hand until play has been pressed once.
//!
//! On the real device, with the master gain at zero so nothing is heard:
//! drags at playback speed for two seconds three times — straight after
//! loading, after a play and a pause, and once more — sampling how far the
//! read head sits behind the pointer every ten milliseconds. READ-ONLY.
//!
//! `cargo run --release -p rbl-deck --example scrubsync -- <audio file>`
// perf-ok: a diagnostic example, not shipped code — it prints and unwraps as
// the other examples in this directory do.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A pointer report every screen frame, as a trackpad gives.
const REPORT: Duration = Duration::from_micros(16_667);

fn drag(label: &str, engine: &rbl_deck::Engine, sample_rate: u32, from_ms: f64) -> f64 {
    let head_ms = || engine.snapshot().a.position_frames as f64 * 1000.0 / f64::from(sample_rate);
    engine.scrub_begin(rbl_deck::Deck::A);
    let mut lags: Vec<(f64, f64)> = Vec::new();
    let started = Instant::now();
    let mut next_report = started;
    let mut pointer = from_ms;
    let mut last_sample = started;
    while started.elapsed() < Duration::from_secs(2) {
        let now = Instant::now();
        if now >= next_report {
            pointer = from_ms + started.elapsed().as_secs_f64() * 1000.0;
            engine.scrub_to_ms(rbl_deck::Deck::A, pointer);
            next_report += REPORT;
        }
        if now.duration_since(last_sample) >= Duration::from_millis(10) {
            lags.push((started.elapsed().as_secs_f64() * 1000.0, pointer - head_ms()));
            last_sample = now;
            if lags.len() % 25 == 1 {
                let a = engine.snapshot().a;
                // perf-ok: example
                println!(
                    "    t={:.0}ms head={} gen={} playing={} loaded={}",
                    started.elapsed().as_secs_f64() * 1000.0,
                    a.position_frames, a.generation, a.playing, a.loaded
                );
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    engine.scrub_end(rbl_deck::Deck::A);
    std::thread::sleep(Duration::from_millis(150));
    let settled: Vec<f64> = lags.iter().filter(|(t, _)| *t > 500.0).map(|(_, l)| *l).collect();
    let mean = settled.iter().sum::<f64>() / settled.len().max(1) as f64;
    let worst = lags.iter().map(|(_, l)| *l).fold(f64::MIN, f64::max);
    // perf-ok: example
    println!("{label}: head behind the pointer by {mean:.0} ms once settled, {worst:.0} ms at worst"); // perf-ok: example
    let early: Vec<String> = lags
        .iter()
        .take_while(|(t, _)| *t < 600.0)
        .step_by(4)
        .map(|(t, l)| format!("{t:.0}:{l:.0}"))
        .collect();
    // perf-ok: example
    println!("  ms:lag over the first 600 ms — {}", early.join(" "));
    println!("  landed at {:.0} ms, pointer at {pointer:.0} ms", head_ms()); // perf-ok: example
    pointer
}

fn main() {
    let Some(path) = std::env::args().nth(1).map(std::path::PathBuf::from) else {
        println!("usage: scrubsync <audio file>");
        return;
    };
    let (tx, rx) = mpsc::channel();
    let events: rbl_deck::EventSink = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    // `--null`: a sink pulled by a thread at the device's cadence instead of
    // the real device, to tell the engine's behaviour from the device's.
    let null = std::env::args().any(|a| a == "--null");
    let engine = if null {
        let (sink_tx, sink_rx) = mpsc::channel();
        let engine = rbl_deck::Engine::with_sink(
            move |render| {
                let sink = Arc::new(rbl_deck::NullSink::new(48_000, render));
                let _ = sink_tx.send(Arc::clone(&sink));
                Ok(sink as Arc<dyn rbl_deck::Sink>)
            },
            &events,
        )
        .expect("engine"); // perf-ok: example
        let sink = sink_rx.recv().expect("sink"); // perf-ok: example
        std::thread::spawn(move || loop {
            let _ = sink.pull(512);
            std::thread::sleep(Duration::from_micros(10_667));
        });
        engine
    } else {
        rbl_deck::Engine::new(&events).expect("engine") // perf-ok: example
    };
    // Silent: the timing is what is measured, and nobody asked to hear it.
    engine.master().set_gain(0.0);
    engine.load(rbl_deck::Deck::A, &path);
    let Ok(rbl_deck::DeckEvent::Loaded { sample_rate, .. }) = rx.recv_timeout(Duration::from_secs(20)) else {
        println!("did not load");
        return;
    };
    // The worker fills its ring after the load; give it the moment the app would.
    std::thread::sleep(Duration::from_millis(300));

    let at = drag("straight after loading", &engine, sample_rate, 0.0);

    engine.play(rbl_deck::Deck::A);
    std::thread::sleep(Duration::from_secs(1));
    engine.pause(rbl_deck::Deck::A);
    std::thread::sleep(Duration::from_millis(500));
    let paused_at = engine.snapshot().a.position_frames as f64 * 1000.0 / f64::from(sample_rate);
    let at = drag("after play and pause", &engine, sample_rate, paused_at.max(at));
    std::thread::sleep(Duration::from_millis(500));
    drag("a third drag", &engine, sample_rate, at);
}
