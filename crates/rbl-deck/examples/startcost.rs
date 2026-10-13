//! How long the device takes to deliver its first callback after `start`,
//! cold and then warm: the first drag on a freshly loaded track is stalled
//! for as long as this is, and this says whether a second start is faster.
//! READ-ONLY; writes silence.
//!
//! `cargo run --release -p rbl-deck --example startcost`
// perf-ok: a diagnostic example, not shipped code — it prints and unwraps as
// the other examples in this directory do.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rbl_deck::{CpalSink, Sink};

fn main() {
    let calls = Arc::new(AtomicU64::new(0));
    let seen = Arc::clone(&calls);
    let sink = CpalSink::open(Box::new(move |out: &mut [f32]| {
        out.fill(0.0);
        seen.fetch_add(1, Ordering::SeqCst);
    }))
    .expect("an output device"); // perf-ok: example
    println!("device at {} Hz", sink.sample_rate()); // perf-ok: example

    for round in 1..=4 {
        let before = calls.load(Ordering::SeqCst);
        let started = Instant::now();
        sink.start().unwrap(); // perf-ok: example
        while calls.load(Ordering::SeqCst) == before {
            if started.elapsed() > Duration::from_secs(3) {
                println!("round {round}: no callback within 3 s"); // perf-ok: example
                break;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        println!("round {round}: first callback {:.1} ms after start", started.elapsed().as_secs_f64() * 1000.0);
        std::thread::sleep(Duration::from_millis(300));
        sink.stop().unwrap(); // perf-ok: example
        // Past the linger, so the stream really is paused before the next start.
        std::thread::sleep(Duration::from_millis(400));
    }
}
