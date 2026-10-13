//! Prints the raw `columns` (page type 16) rows of a real export.pdb — the
//! browse-menu names a player reads — to see whether order or visibility is
//! encoded there. READ-ONLY.
//!
//! `cargo run -p rbl-pdb --example columns -- <export.pdb>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        println!("usage: columns <export.pdb>");
        return;
    };
    let bytes = std::fs::read(&path).expect("read");
    let pdb = rbl_pdb::Pdb::parse(&bytes).expect("parse");
    for table in &pdb.tables {
        if !matches!(table.page_type, rbl_pdb::PageType::Columns | rbl_pdb::PageType::Colors) {
            continue;
        }
        println!("== {}", table.page_type.name());
        for row in pdb.rows(table) {
            let raw = &bytes[row.offset..(row.offset + 40).min(bytes.len())];
            let hex: Vec<String> = raw.iter().map(|b| format!("{b:02x}")).collect();
            println!("  @{:06x} {}", row.offset, hex.join(" "));
        }
    }
}
