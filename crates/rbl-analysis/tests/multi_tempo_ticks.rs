//! Sample-exact counterparts of scripts/analysis-artifacts/generate-grid-fixtures.py.
#![allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout)]

use rbl_analysis::{analyse_with, AnalysisPreset};

const RATE: u32 = 48_000;

#[test]
fn every_tick_has_a_grid_marker_through_cuts_and_ramps() {
    let cases = [
        ("up on bar", 128.0, 174.0, 129, 129, false),
        ("up mid bar", 128.0, 174.0, 130, 130, false),
        ("long linear ramp", 128.0, 174.0, 65, 129, false),
        ("short linear ramp", 128.0, 174.0, 65, 81, false),
        ("down on bar", 174.0, 128.0, 129, 129, false),
        ("curved ramp", 128.0, 174.0, 65, 129, true),
    ];
    let mut failures = Vec::new();
    for (name, initial, final_bpm, from, to, curved) in cases {
        let mut audio = vec![0.0_f32; RATE as usize * 120];
        let mut expected = Vec::new();
        let mut seconds = 0.0;
        while seconds < 120.0 {
            let beat = expected.len() + 1;
            let bpm = if beat < from { initial } else if beat >= to { final_bpm } else {
                let u = (beat - from) as f64 / (to - from) as f64;
                let progress = if curved { u * u * (3.0 - 2.0 * u) } else { u };
                initial + (final_bpm - initial) * progress
            };
            let sample = (seconds * f64::from(RATE)).round() as usize;
            if sample >= audio.len() { break; }
            expected.push(sample as f64 * 1000.0 / f64::from(RATE));
            for j in 0..240.min(audio.len() - sample) {
                let value = 24_000.0 * (-(j as f64) / 45.0).exp()
                    * (2.0 * std::f64::consts::PI * 4_000.0 * j as f64 / f64::from(RATE)).sin();
                audio[sample + j] = value.round() as i16 as f32 / 32_768.0;
            }
            seconds += 60.0 / bpm;
        }
        let grid = analyse_with(&audio, RATE, AnalysisPreset::Rbxport.options()).tempo;
        let misses: Vec<_> = expected.iter().enumerate().filter_map(|(i, &time)| {
            let error = grid.beats.get(i).map_or(f64::INFINITY, |b| (f64::from(b.time_ms) - time).abs());
            (error > 2.0 || grid.beats.get(i).is_some_and(|b| b.beat_number != (i % 4 + 1) as u16))
                .then_some((i + 1, time, error))
        }).collect();
        println!("{name}: {} ticks, {} markers, {} misses; first {:?}; segments {:?}", expected.len(), grid.beats.len(), misses.len(),
            &misses[..misses.len().min(5)], grid.segments.iter().map(|s| (s.start_secs(), s.bpm(), s.beats())).collect::<Vec<_>>());
        if !misses.is_empty() || grid.beats.len() != expected.len() || grid.beats[0].time_ms != 0
            || (grid.bpm - initial).abs() > 0.01 {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "tick grids failed: {failures:?}");
}
