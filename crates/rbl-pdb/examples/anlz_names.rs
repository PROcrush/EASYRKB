//! READ-ONLY: prints each track's stick id, analysis folder, file name and
//! title from an `export.pdb`, tab-separated, for working out how rekordbox
//! names the `USBANLZ` folders.
//!
//! `cargo run -q -p rbl-pdb --example anlz_names -- <export.pdb>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
fn main() {
    let path = std::env::args().nth(1).expect("a path to export.pdb");
    let bytes = std::fs::read(path).expect("read");
    let pdb = rbl_pdb::Pdb::parse(&bytes).expect("parse");
    let table = pdb.table(rbl_pdb::PageType::Tracks).expect("tracks");
    for t in pdb.track_rows(table) {
        println!("{}\t{}\t{}\t{}\t{}\t{}\t{}", t.id, t.analyze_path, t.filename, t.file_size, t.duration_sec, t.title, t.file_path);
    }
}
