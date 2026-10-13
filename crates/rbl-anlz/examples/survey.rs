//! READ-ONLY survey of the installed library's ANLZ files.
//! `cargo run --release -p rbl-anlz --example survey -- [max_files]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let limit: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let share = match dirs::home_dir() {
        Some(h) => h.join("Library/Pioneer/rekordbox/share/PIONEER/USBANLZ"),
        None => { println!("no home dir"); return; }
    };
    if !share.exists() {
        println!("no ANLZ tree at {}", share.display());
        return;
    }

    let mut files = Vec::new();
    collect(&share, &mut files, limit * 3);

    let mut by_ext: BTreeMap<String, usize> = BTreeMap::new();
    let mut tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut parsed = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut beat_files = 0usize;
    let mut total_beats = 0usize;
    let mut cue_files = 0usize;
    let mut total_cues = 0usize;
    let mut waveform_bytes = 0usize;

    for path in files.iter().take(limit) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_uppercase();
        *by_ext.entry(ext).or_default() += 1;
        match rbl_anlz::Anlz::read(path) {
            Ok(anlz) => {
                parsed += 1;
                for section in &anlz.sections {
                    *tags.entry(section.tag.to_string()).or_default() += 1;
                    if section.waveform().is_some() {
                        waveform_bytes += section.payload.len();
                    }
                }
                if let Some(beats) = anlz.beat_grid() {
                    if !beats.is_empty() { beat_files += 1; total_beats += beats.len(); }
                }
                for section in &anlz.sections {
                    if section.is_cue_list() && !section.payload.is_empty() {
                        cue_files += 1;
                        total_cues += 1;
                    }
                }
            }
            Err(e) => failed.push((path.display().to_string(), e.to_string())),
        }
    }

    println!("scanned {} files under {}", parsed + failed.len(), share.display());
    println!("  by extension: {by_ext:?}");
    println!("  parsed ok: {parsed}   failed: {}", failed.len());
    println!("  beat grids: {beat_files} files, {total_beats} beats total");
    println!("  cue lists:  {cue_files} files, {total_cues} cues total");
    println!("  waveform payload: {:.1} MB", waveform_bytes as f64 / 1_048_576.0);
    println!("  tag frequency:");
    let mut sorted: Vec<_> = tags.into_iter().collect();
    sorted.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (tag, n) in sorted { println!("    {tag}  {n}"); }
    for (path, err) in failed.iter().take(5) { println!("  FAILED {path}: {err}"); }
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>, cap: usize) {
    if out.len() >= cap { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() { collect(&path, out, cap); }
        else if matches!(path.extension().and_then(|e| e.to_str()), Some("DAT" | "EXT" | "2EX")) {
            out.push(path);
            if out.len() >= cap { return; }
        }
    }
}
