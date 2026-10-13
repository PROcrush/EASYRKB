//! Reads a real export.pdb. `cargo run -p rbl-pdb --example dump -- <path>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        println!("usage: dump <export.pdb>");
        return;
    };
    let bytes = std::fs::read(&path).expect("read");
    let pdb = match rbl_pdb::Pdb::parse(&bytes) {
        Ok(p) => p,
        Err(e) => { println!("parse failed: {e}"); return; }
    };
    println!("{} ({} bytes)", path, bytes.len());
    println!("  page size {}  tables {}", pdb.page_size, pdb.tables.len());
    println!("  census:");
    for (name, n) in pdb.census() {
        println!("    {name:<20} {n}");
    }

    for table in &pdb.tables {
        match table.page_type {
            rbl_pdb::PageType::Artists | rbl_pdb::PageType::Genres | rbl_pdb::PageType::Keys => {
                let rows = pdb.named_rows(table);
                println!("  {} sample:", table.page_type.name());
                for r in rows.iter().take(5) {
                    println!("    id={} {:?}", r.id, r.name);
                }
            }
            rbl_pdb::PageType::Tracks => {
                let rows = pdb.track_rows(table);
                println!("  tracks sample ({} rows):", rows.len());
                for t in rows.iter().take(8) {
                    println!(
                        "    id={:<5} bpm={:<7.2} {:>4}s  {:?}",
                        t.id,
                        f64::from(t.tempo_x100) / 100.0,
                        t.duration_sec,
                        t.title,
                    );
                }
                if let Some(t) = rows.first() {
                    println!("    first row paths: file={:?}", t.file_path);
                    println!("                     analyze={:?}", t.analyze_path);
                    println!("                     filename={:?}", t.filename);
                }
            }
            rbl_pdb::PageType::PlaylistTree => {
                for n in pdb.playlist_nodes(table).iter().take(10) {
                    println!("    playlist id={} parent={} folder={} {:?}", n.id, n.parent_id, n.is_folder, n.name);
                }
            }
            rbl_pdb::PageType::PlaylistEntries => {
                for e in pdb.playlist_entries(table).iter().take(8) {
                    println!("    entry #{} track={} playlist={}", e.entry_index, e.track_id, e.playlist_id);
                }
            }
            rbl_pdb::PageType::Colors => {
                for c in pdb.named_rows(table).iter().take(9) {
                    println!("    color id={} {:?}", c.id, c.name);
                }
            }
            _ => {}
        }
    }
}
