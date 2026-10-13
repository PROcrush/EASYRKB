//! Does the master limiter do its job on real music, and does it pump?
//!
//! Plays one file on both decks at once — the sum that clipped — and pulls
//! ten seconds through the engine with the limiter on and again with it off.
//! What is measured is what an ear would have judged: the loudest sample the
//! device was given, how many samples sat on the rail, clicks (a step that
//! does not belong to the music around it — `scrubcheck`'s measure), and how
//! much the limiter's gain moved, which is what pumping is.
//!
//! Run: `cargo run --release -p rbl-deck --example limitcheck -- <file A> [file B] [release ms]`
//!
//! One file is the worst case — the same peaks summed in phase. Two is a mix.

// perf-ok: a probe run by hand, as scrubcheck is — it prints and it panics on
// a setup it cannot make, and none of it ships.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{mpsc, Arc};
use std::time::Duration;

const BUFFER: usize = 512;
const SECONDS: f64 = 10.0;
const CLICK_RATIO: f32 = 12.0;
const CLICK_NEIGHBOURHOOD: usize = 96;

fn clicks(stream: &[f32]) -> u32 {
    let frames: Vec<f32> = stream.chunks_exact(2).map(|f| f[0]).collect();
    let steps: Vec<f32> = frames.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let mut found = 0;
    for i in CLICK_NEIGHBOURHOOD..steps.len().saturating_sub(CLICK_NEIGHBOURHOOD) {
        let around: f32 = steps[i - CLICK_NEIGHBOURHOOD..i]
            .iter()
            .chain(&steps[i + 1..i + 1 + CLICK_NEIGHBOURHOOD])
            .sum::<f32>()
            / (CLICK_NEIGHBOURHOOD * 2) as f32;
        if around > 1e-4 && steps[i] > around * CLICK_RATIO {
            found += 1;
        }
    }
    found
}

fn db(value: f32) -> f32 {
    20.0 * value.max(1e-9).log10()
}

struct Run {
    peak: f32,
    on_rail: usize,
    clicks: u32,
    /// Peak reduction each tick, in dB, as the meter would have shown.
    reductions: Vec<f32>,
}

fn run(a: &std::path::Path, b: &std::path::Path, release_ms: f32, enabled: bool) -> Option<Run> {
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
    // perf-ok: a probe; no engine means nothing to measure.
    .expect("engine");
    // perf-ok: as above.
    let sink = sink_rx.recv().expect("sink");
    engine.limiter().set_enabled(enabled);
    engine.limiter().set_release_ms(release_ms);
    engine.load(rbl_deck::Deck::A, a);
    engine.load(rbl_deck::Deck::B, b);
    let mut loaded = 0;
    while loaded < 2 {
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(rbl_deck::DeckEvent::Loaded { .. }) => loaded += 1,
            _ => {
                // perf-ok: a probe reports to the terminal.
                println!("did not load");
                return None;
            }
        }
    }
    // Into the track, where the drop is, rather than the intro.
    engine.seek_ms(rbl_deck::Deck::A, 60_000.0);
    engine.seek_ms(rbl_deck::Deck::B, 60_000.0);
    std::thread::sleep(Duration::from_millis(300));
    engine.play(rbl_deck::Deck::A);
    engine.play(rbl_deck::Deck::B);

    let rate = engine.sample_rate();
    let period = Duration::from_secs_f64(BUFFER as f64 / f64::from(rate));
    let wanted = (SECONDS * f64::from(rate)) as usize;
    let mut stream: Vec<f32> = Vec::with_capacity(wanted * 2);
    let mut reductions = Vec::new();
    let mut since_tick = 0_usize;
    while stream.len() / 2 < wanted {
        stream.extend_from_slice(&sink.pull(BUFFER));
        since_tick += BUFFER;
        // The meter's own beat: 33 ms.
        if since_tick >= rate as usize / 30 {
            since_tick = 0;
            reductions.push(engine.master().reduction_db());
        }
        std::thread::sleep(period);
    }
    // Skip the fade-in.
    let body = &stream[rate as usize * 2..];
    let peak = body.iter().fold(0.0_f32, |a, s| a.max(s.abs()));
    let on_rail = body.iter().filter(|s| s.abs() >= 0.999).count();
    Some(Run { peak, on_rail, clicks: clicks(body), reductions })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(a) = args.first().map(std::path::PathBuf::from) else {
        // perf-ok: a probe reports to the terminal.
        println!("usage: limitcheck <file A> [file B] [release ms]");
        return;
    };
    let b = args.get(1).map_or_else(|| a.clone(), std::path::PathBuf::from);
    let release_ms = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100.0);
    for enabled in [false, true] {
        let Some(run) = run(&a, &b, release_ms, enabled) else { return };
        let label = if enabled { "limiter on " } else { "limiter off" };
        println!(
            "{label}: peak {:.3} ({:+.1} dBFS), {} samples on the rail, {} clicks",
            run.peak,
            db(run.peak),
            run.on_rail,
            run.clicks
        );
        if enabled {
            let ticks = run.reductions.len().max(1) as f32;
            let mean = run.reductions.iter().sum::<f32>() / ticks;
            let most = run.reductions.iter().copied().fold(0.0_f32, f32::max);
            let working = run.reductions.iter().filter(|r| **r > 0.1).count();
            // Tick-to-tick swing is what pumping is: a limiter that ducks 6 dB
            // on a kick and is back to 0 before the next reads as breathing.
            let swing = run
                .reductions
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0_f32, f32::max);
            println!(
                "  reduction: mean {mean:.1} dB, most {most:.1} dB, working {:.0}% of ticks, largest tick-to-tick swing {swing:.1} dB",
                working as f32 / ticks * 100.0
            );
        }
    }
}
