//! READ-ONLY: how much a snapshot saves on the installed library.
//! `cargo run --release -p rbl-index --example cachebench`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

fn main() {
    let path = std::env::temp_dir().join("rbl-cachebench.snapshot");
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => { println!("cannot open library: {e}"); return; }
    };
    let master = db.location().master_db.clone();
    let version = db.schema().db_version.and_then(|v| u32::try_from(v).ok()).unwrap_or(0);
    let content = rbl_index::content_version(&db).unwrap_or(0);
    let fp = rbl_index::cache::Fingerprint::of(&master, version, content).expect("fingerprint");

    let t0 = Instant::now();
    let (library, _) = rbl_index::load(&db).expect("load");
    println!("normal load        {:>5} ms  ({} tracks)", t0.elapsed().as_millis(), library.len());

    let t1 = Instant::now();
    rbl_index::cache::save(&path, &library, fp).expect("save");
    let size = std::fs::metadata(&path).map_or(0, |m| m.len());
    println!("write snapshot     {:>5} ms  ({:.1} MB)", t1.elapsed().as_millis(), size as f64 / 1_048_576.0);

    let t2 = Instant::now();
    let restored = rbl_index::cache::load(&path, fp).expect("load from cache");
    println!("load from snapshot {:>5} ms  ({} tracks)", t2.elapsed().as_millis(), restored.len());

    assert_eq!(restored.len(), library.len());
    assert_eq!(restored.ids, library.ids);
    let _ = std::fs::remove_file(&path);
}
