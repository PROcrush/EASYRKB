//! Do the extended cue tags in a real USB export carry colours? READ-ONLY.
//!
//! `ColorTableIndex` in `master.db` decides what colour rekordbox draws a cue,
//! and its palette is not in the database, the skins, or the desktop analysis
//! files. A USB export's `PCO2` tags are the last place a colour could be
//! recorded outside the binary — a player has to draw them from something.
//!
//! `cargo run -p rbl-anlz --example cue_colours -- <USBANLZ dir>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

fn main() {
    let Some(root) = std::env::args().nth(1).map(PathBuf::from) else {
        println!("usage: cue_colours <USBANLZ directory>");
        return;
    };

    let mut files = Vec::new();
    collect(&root, &mut files);
    println!("{} analysis files under {}\n", files.len(), root.display());

    for path in &files {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let Ok(file) = rbl_anlz::parse(&bytes) else { continue };
        for section in &file.sections {
            let tag = section.tag.as_str();
            if tag != "PCO2" && tag != "PCOB" {
                continue;
            }
            println!("{} — {tag}: header {} bytes, payload {} bytes",
                     path.file_name().unwrap_or_default().to_string_lossy(),
                     section.header.len(), section.payload.len());
            // The header carries the entry count; the payload the entries.
            // Dumping the first bytes is enough to see whether colours are
            // present at all before decoding anything.
            let head: Vec<String> =
                section.header.iter().take(16).map(|b| format!("{b:02x}")).collect();
            let body: Vec<String> =
                section.payload.iter().take(48).map(|b| format!("{b:02x}")).collect();
            println!("    header  {}", head.join(" "));
            println!("    payload {}", body.join(" "));
        }
    }
}

fn collect(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}
