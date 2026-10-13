//! READ-ONLY: what the app's own library snapshot holds — the histories in
//! particular, which formats before 4 left out.
//! `cargo run --release -p rbl-index --example snapshotcheck -- <snapshot path>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let path = std::env::args().nth(1).expect("snapshot path");
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let content = rbl_index::content_version(&db).unwrap_or(0);
    let schema = db.schema().db_version.and_then(|v| u32::try_from(v).ok()).unwrap_or(0);
    let fp = rbl_index::cache::Fingerprint::of(&db.location().master_db, schema, content).expect("fingerprint");
    println!("format {} content {}", fp.format, content);
    match rbl_index::cache::load(std::path::Path::new(&path), fp) {
        Some(lib) => {
            let h = lib.histories();
            println!("snapshot loads: {} tracks, {} playlists, {} history nodes", lib.len(), lib.playlists().len(), h.len());
            for i in 0..h.len().min(6) { println!("  {} folder={}", h.name(i), h.is_folder(i)); }
        }
        None => println!("snapshot refused (fingerprint or format mismatch): the app would re-read the database"),
    }
}
