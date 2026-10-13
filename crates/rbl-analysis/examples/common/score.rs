//! Scoring one grid against rekordbox's, shared by the `golden` and
//! `multibpm` rigs (`#[path]`-included by each).
#![allow(dead_code)]

use rbl_analysis::tempo::Beat;

/// How far from rekordbox's BPM still counts.
pub const BPM_TOLERANCE: f64 = 0.05;
/// How far a beat may sit from rekordbox's and still be the same beat.
pub const MS_TOLERANCE: f64 = 25.0;
/// The share of rekordbox's beats that must be matched for the grid to pass.
pub const GRID_PASS: f64 = 0.98;

/// Signed distance to the nearest multiple of `period`.
pub fn wrap(offset: f64, period: f64) -> f64 {
    if period <= 0.0 {
        return offset;
    }
    let d = offset.rem_euclid(period);
    if d > period / 2.0 { d - period } else { d }
}

/// The offset of our grid from rekordbox's, at rekordbox's first downbeat.
///
/// Compared where the two grids overlap: rekordbox's first downbeat is the
/// anchor, and the nearest downbeat of ours to it gives the bar offset. Taken
/// modulo a bar of rekordbox's tempo there, so "we chose the wrong beat of
/// the bar" shows up as a multiple of a beat, and "we chose the right beat a
/// little late" as a small number. Also returned: the same thing modulo a
/// beat, and which of rekordbox's beat numbers our downbeat coincides with.
pub fn offsets(ours: &[Beat], rb: &[Beat]) -> (f64, f64, u16) {
    let Some(anchor) = rb.iter().find(|b| b.beat_number == 1) else {
        return (f64::INFINITY, f64::INFINITY, 0);
    };
    let tempo = f64::from(anchor.tempo_x100) / 100.0;
    if tempo <= 0.0 {
        return (f64::INFINITY, f64::INFINITY, 0);
    }
    let beat_ms = 60_000.0 / tempo;
    let bar_ms = beat_ms * 4.0;
    let anchor_ms = f64::from(anchor.time_ms);
    // Our downbeat nearest rekordbox's anchor, by bar-wrapped distance.
    let mut best: Option<(f64, f64)> = None;
    for beat in ours.iter().filter(|b| b.beat_number == 1) {
        let raw = f64::from(beat.time_ms) - anchor_ms;
        // Only downbeats within a few bars of the anchor: further away, a
        // tempo difference would leak into the phase.
        if raw.abs() > bar_ms * 2.5 {
            continue;
        }
        let wrapped = wrap(raw, bar_ms);
        if best.is_none_or(|(b, _)| wrapped.abs() < b.abs()) {
            best = Some((wrapped, raw));
        }
    }
    let Some((down, _)) = best else {
        return (f64::INFINITY, f64::INFINITY, 0);
    };
    let beat = wrap(down, beat_ms);
    // Which rekordbox beat number our downbeat sits on: 1 when the downbeats
    // agree, else 2..=4.
    let steps = ((down - beat) / beat_ms).round() as i64;
    let beat_in_bar = (steps.rem_euclid(4) + 1) as u16;
    (down, beat, beat_in_bar)
}

/// The share of rekordbox's beats that one of ours matches, in time and number.
pub fn grid_match(ours: &[Beat], rb: &[Beat]) -> f64 {
    if rb.is_empty() || ours.is_empty() {
        return 0.0;
    }
    let mut j = 0usize;
    let mut matched = 0usize;
    for beat in rb {
        let t = f64::from(beat.time_ms);
        // Advance to the first of ours not before this beat's window.
        while j + 1 < ours.len() && f64::from(ours[j].time_ms) < t - MS_TOLERANCE {
            j += 1;
        }
        let hit = ours[j.saturating_sub(1)..(j + 2).min(ours.len())]
            .iter()
            .any(|o| (f64::from(o.time_ms) - t).abs() <= MS_TOLERANCE && o.beat_number == beat.beat_number);
        if hit {
            matched += 1;
        }
    }
    matched as f64 / rb.len() as f64
}

/// One stretch of a grid at one tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// The first beat of the run.
    pub start_ms: u32,
    /// Its beat number, 1..=4.
    pub beat_number: u16,
    pub tempo_x100: u16,
    pub beats: usize,
}

/// A grid as its tempo runs, in order: a constant-tempo track has one, a
/// DJ edit two, a bar-by-bar ramp one per bar.
pub fn runs(grid: &[Beat]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for beat in grid {
        match out.last_mut() {
            Some(run) if run.tempo_x100 == beat.tempo_x100 => run.beats += 1,
            _ => out.push(Run { start_ms: beat.time_ms, beat_number: beat.beat_number, tempo_x100: beat.tempo_x100, beats: 1 }),
        }
    }
    out
}

/// The runs as one line: `136.00 @ 0.512s ×245 | 174.00 @ 129.412s ×612`,
/// eliding the middle of a long ramp.
pub fn runs_line(grid: &[Beat]) -> String {
    let runs = runs(grid);
    let show = |r: &Run| format!("{:.2} @ {:.3}s b{} ×{}", f64::from(r.tempo_x100) / 100.0, f64::from(r.start_ms) / 1000.0, r.beat_number, r.beats);
    if runs.len() <= 8 {
        return runs.iter().map(show).collect::<Vec<_>>().join(" | ");
    }
    let head: Vec<String> = runs[..3].iter().map(show).collect();
    let tail: Vec<String> = runs[runs.len() - 3..].iter().map(show).collect();
    format!("{} | … {} runs … | {}", head.join(" | "), runs.len() - 6, tail.join(" | "))
}
