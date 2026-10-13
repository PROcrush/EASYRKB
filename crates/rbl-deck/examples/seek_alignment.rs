//! Compare seeked PCM with sequential decoding, without opening an audio device.
// Diagnostic only: reads the file and compares PCM; no device or library writes.
#![allow(
    clippy::pedantic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used
)]
#[allow(dead_code)]
#[path = "../src/decode.rs"]
mod decode;
use rbl_deck::{DeckError, Result};
fn main() {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().expect("audio path"));
    let requested: Vec<f64> = args
        .map(|value| value.parse().expect("seek position in seconds"))
        .collect();
    let positions = if requested.is_empty() {
        vec![0.0, 10.0, 24.4, 25.0, 26.4]
    } else {
        requested
    };
    assert!(positions
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0));
    let reference_seconds = positions.iter().copied().fold(0.0_f64, f64::max).ceil() as usize + 4;
    for rate in [44100, 48000] {
        let mut linear = decode::Streamer::open(&path, rate).unwrap();
        let mut reference = vec![0f32; rate as usize * reference_seconds * 2];
        let n = linear.fill(&mut reference).unwrap();
        println!(
            "rate {rate}, reference {n} frames, total {}",
            linear.total_frames()
        );
        for threshold in [0.0001, 0.001, 0.01] {
            let onset = reference[..n * 2]
                .chunks_exact(2)
                .position(|frame| frame.iter().any(|v| v.abs() >= threshold));
            println!(
                "first PCM above {threshold}: {onset:?} frames ({:.3} ms)",
                onset.unwrap_or(0) as f64 * 1000.0 / f64::from(rate)
            );
        }
        for &seconds in &positions {
            let at = (seconds * rate as f64) as usize;
            if at + rate as usize * 2 >= n {
                continue;
            }
            let mut seek = decode::Streamer::open(&path, rate).unwrap();
            let started = std::time::Instant::now();
            seek.seek(at as u64).unwrap();
            let seek_ms = started.elapsed().as_secs_f64() * 1000.0;
            let mut samples = vec![0f32; rate as usize * 3 * 2];
            seek.fill(&mut samples).unwrap();
            // Include the attack immediately after seeking, not only audio
            // one second later; this is the region used by cue auditioning.
            let start = 0;
            let count = 4096;
            let mut best = (f64::INFINITY, 0i32);
            for shift in -2048i32..=2048 {
                if at as i64 + i64::from(shift) < 0 {
                    continue;
                }
                let mut error = 0f64;
                for i in (start..start + count).step_by(16) {
                    let a = samples[i * 2];
                    let j = (at + i) as i64 + shift as i64;
                    let b = reference[j as usize * 2];
                    error += (a as f64 - b as f64).powi(2);
                }
                if error < best.0 {
                    best = (error, shift);
                }
            }
            println!("seek {seconds} ({seek_ms:.2} ms): best reference offset {} frames ({:.3} ms), SSE {:.6}",best.1,best.1 as f64*1000.0/rate as f64,best.0);
        }
    }
}
