//! Scrub audio must keep reaching the UI meters while the transport is paused.
#![allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
use rbl_deck::{Deck, NullSink, Sink};
use rbxport_lib::{commands, player::Player};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Listener, Manager};

fn tone(path: &std::path::Path) {
    let rate = 44_100_u32;
    let bytes = rate * 4 * 4;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    for frame in 0..rate * 4 {
        let sample =
            ((frame as f32 * 330.0 * std::f32::consts::TAU / rate as f32).sin() * 16_000.0) as i16;
        for _ in 0..2 {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
    }
    std::fs::write(path, wav).unwrap();
}

#[test]
fn paused_scrubs_on_either_deck_keep_emitting_audio_peaks_until_released() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tone.wav");
    tone(&path);
    let sink = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&sink);
    let player = Player::with_sink(Box::new(move |render, _, _| {
        let output = Arc::new(NullSink::new(44_100, render));
        *captured.lock().unwrap() = Some(Arc::clone(&output));
        Ok(output as Arc<dyn Sink>)
    }));
    let app = tauri::test::mock_app();
    app.manage(Arc::new(player));
    let player = app.state::<Arc<Player>>();
    let engine = player.engine(app.handle()).unwrap();
    let peaks = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&peaks);
    app.listen("deck:meters", move |event| {
        let value: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
        received
            .lock()
            .unwrap()
            .push((Instant::now(), value["peakLeft"].as_f64().unwrap()));
    });
    for (name, deck) in [("a", Deck::A), ("b", Deck::B)] {
        engine.load(deck, &path);
        let deadline = Instant::now() + Duration::from_secs(3);
        while !(if deck == Deck::A {
            engine.snapshot().a.loaded
        } else {
            engine.snapshot().b.loaded
        }) {
            assert!(Instant::now() < deadline, "the tone should load");
            std::thread::sleep(Duration::from_millis(5));
        }
        peaks.lock().unwrap().clear();
        tauri::async_runtime::block_on(commands::deck_scrub_begin(
            app.handle().clone(),
            app.state(),
            name.into(),
        ))
        .unwrap();
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(650) {
            // Drag forwards then backwards, without ever pressing Play.
            let t = started.elapsed().as_secs_f64();
            engine.scrub_to_ms(
                deck,
                1000.0
                    + if t < 0.325 {
                        t * 1200.0
                    } else {
                        (0.65 - t) * 1200.0
                    },
            );
            sink.lock().unwrap().as_ref().unwrap().pull(512);
            std::thread::sleep(Duration::from_millis(10));
        }
        let late_peak = peaks.lock().unwrap().iter().any(|(time, peak)| {
            time.saturating_duration_since(started) > Duration::from_millis(400) && *peak > 0.01
        });
        tauri::async_runtime::block_on(commands::deck_scrub_end(
            app.handle().clone(),
            app.state(),
            name.into(),
        ))
        .unwrap();
        assert!(
            !engine.any_playing(),
            "scrubbing must leave transport paused"
        );
        assert!(
            late_peak,
            "paused deck {name} stopped publishing scrub audio to the meters"
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while player.ticking().load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                Instant::now() < deadline,
                "the ticker must stop after release"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
