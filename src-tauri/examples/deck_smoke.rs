//! Plays tracks from the REAL library through the engine, read-only, without a
//! webview. `cargo run --release -p rbxport --example deck_smoke`
//!
//! This is how playback is checked against the user's own files: the engine's
//! own tests use generated WAVs, and what those cannot tell you is whether a
//! track id from the index resolves to a file that is still on disk and whether
//! symphonia decodes the formats this library actually holds.
//!
//! Nothing is heard: the sink has no device behind it, and the audio is
//! measured rather than played.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rbl_deck::{Deck, DeckEvent, Engine, NullSink, Sink};

/// Tracks to try. Enough to cross a few formats without reading the library.
const SAMPLE: usize = 8;

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let (library, stats) = rbl_index::load(&db).expect("index");
    println!("library: {} tracks", stats.tracks);

    // The device's own rate, so what this measures is what the app will do. A
    // machine at 96 kHz resamples every 44.1 kHz file in the library.
    let rate = match rbl_deck::CpalSink::open(Box::new(|out: &mut [f32]| out.fill(0.0))) {
        Ok(sink) => {
            println!("default device: {} Hz", sink.sample_rate());
            sink.sample_rate()
        }
        Err(e) => {
            println!("no audio device ({e}); measuring at 44,100 Hz");
            44_100
        }
    };

    let mut checked = 0;
    let mut missing = 0;
    let mut played = 0;

    for row in 0..library.len() {
        if checked >= SAMPLE {
            break;
        }
        let id = library.ids.get(row).copied().unwrap_or(0).to_string();
        let Some(path) = library.audio_path_of(&id) else { continue };
        if path.is_empty() {
            continue;
        }
        let file = std::path::PathBuf::from(path);
        if !file.exists() {
            missing += 1;
            continue;
        }
        checked += 1;

        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&events);
        let slot: Arc<Mutex<Option<Arc<NullSink>>>> = Arc::new(Mutex::new(None));
        let put = Arc::clone(&slot);
        let sink_events: rbl_deck::EventSink =
            Arc::new(move |e: DeckEvent| seen.lock().unwrap().push(format!("{e:?}")));
        let engine = Engine::with_sink(
            move |render| {
                let sink = Arc::new(NullSink::new(rate, render));
                *put.lock().unwrap() = Some(Arc::clone(&sink));
                Ok(sink as Arc<dyn Sink>)
            },
            &sink_events,
        )
        .unwrap();
        let sink = slot.lock().unwrap().clone().unwrap();

        engine.load(Deck::A, &file);
        let deadline = Instant::now() + Duration::from_secs(20);
        while events.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let total = engine.snapshot().a.total_frames;

        // From the middle rather than the start: a track that opens on silence
        // or a long fade would read as a failed decode.
        engine.play(Deck::A);
        engine.seek_frames(Deck::A, total / 2);
        let started = Instant::now();
        let want = total / 2 + u64::from(rate) * 2;
        let mut peak = 0.0_f32;
        let stop = Instant::now() + Duration::from_secs(20);
        while engine.snapshot().a.position_frames < want && Instant::now() < stop {
            for sample in sink.pull(4096) {
                peak = peak.max(sample.abs());
            }
        }
        let elapsed = started.elapsed().as_secs_f64();
        if peak > 0.01 {
            played += 1;
        }
        println!(
            "{:>8} {:>6.1} s  peak {peak:.3}  seek and 2 s of audio in {:>5.0} ms  {}  {}",
            id,
            total as f64 / f64::from(rate),
            elapsed * 1000.0,
            if peak > 0.01 { "ok    " } else { "SILENT" },
            file.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        );
        let first = events.lock().unwrap().first().cloned().unwrap_or_default();
        if first.contains("Error") {
            println!("         {first}");
        }
    }
    println!("{played}/{checked} played, {missing} of the files tried were not on disk");
}
