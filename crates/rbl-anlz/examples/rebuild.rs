//! READ-ONLY fidelity check: parse real ANLZ files, re-emit them, compare.
//!
//! Byte-identical output proves the reader lost nothing and the writer adds
//! nothing — the property an export depends on.
//!
//! `cargo run --release -p rbl-anlz --example rebuild -- [count]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    let limit: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let Some(home) = dirs::home_dir() else { println!("no home dir"); return };
    let share = home.join("Library/Pioneer/rekordbox/share/PIONEER/USBANLZ");
    if !share.exists() {
        println!("no ANLZ tree at {}", share.display());
        return;
    }

    let mut files = Vec::new();
    collect(&share, &mut files, limit);

    let mut identical = 0usize;
    let mut differing: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let mut failed = 0usize;

    for path in &files {
        let Ok(original) = std::fs::read(path) else { failed += 1; continue };
        let Ok(parsed) = rbl_anlz::parse(&original) else { failed += 1; continue };

        let rebuilt = parsed.to_bytes();

        if rebuilt == original {
            identical += 1;
        } else {
            // Which tags does the file hold? That is the useful grouping when
            // output differs.
            let tags: Vec<String> = parsed.sections.iter().map(|s| s.tag.to_string()).collect();
            *differing.entry(tags.join("+")).or_default() += 1;
            if examples.len() < 3 {
                examples.push(format!(
                    "{}: {} bytes in, {} out",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    original.len(),
                    rebuilt.len()
                ));
            }
        }
    }

    let total = identical + differing.values().sum::<usize>();
    println!("rebuilt {total} files ({failed} unreadable)");
    println!("  byte-identical: {identical} / {total}  ({:.1}%)",
             identical as f64 / total.max(1) as f64 * 100.0);
    if !differing.is_empty() {
        println!("  differing, by tag set:");
        let mut sorted: Vec<_> = differing.into_iter().collect();
        sorted.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (tags, n) in sorted.into_iter().take(8) {
            println!("    {n:>6}  {tags}");
        }
        for e in &examples { println!("    e.g. {e}"); }
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>, cap: usize) {
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
