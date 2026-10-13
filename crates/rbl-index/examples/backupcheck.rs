//! Does a backup of the real library reopen as a library? READ-ONLY.
//!
//! The warning at the top of `TODO.md` asks for exactly this. The writer takes
//! a copy before the first write of a session, and that copy has only ever
//! been checked against a fixture — 40 tracks in a temporary directory. The
//! real thing is 1.8 GB of SQLCipher with a live WAL beside it, and "the
//! backup works" is not a claim worth making on the small one.
//!
//! So this does what the writer does: `std::fs::copy` of the database and then
//! its `-wal` and `-shm` sidecars, the same call in the same order, then opens
//! the copy and reads the whole library out of it. The original is never
//! opened for writing and never touched; the copy is deleted unless `--keep`
//! is given.
//!
//! Harsher than the real case on purpose. The writer only ever copies once
//! rekordbox has been quit; this copies whether it is running or not, so a
//! copy that survives here survives the quiet case.
//!
//! `cargo run --release -p rbl-index --example backupcheck -- [--keep]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    let keep = std::env::args().any(|a| a == "--keep");
    let Ok(location) = rbl_db::detect() else {
        println!("no installed library found");
        return;
    };
    let source = location.master_db.clone();
    println!("source: {} ({:.2} GB)", source.display(), size_gb(&source));
    println!("rekordbox running: {}", rbl_db::is_rekordbox_running());

    let into = std::env::temp_dir().join(format!("rbl-backupcheck-{}", std::process::id()));
    std::fs::create_dir_all(&into).expect("scratch dir");
    let copy = into.join("master.db");

    // The writer's own order: the database, then the sidecars. A database
    // whose WAL is left behind is a database missing whatever was in it, which
    // is the whole reason the sidecars go with it.
    let started = Instant::now();
    std::fs::copy(&source, &copy).expect("copy the database");
    let mut sidecars = 0;
    for suffix in ["-wal", "-shm"] {
        let from = with_suffix(&source, suffix);
        if from.exists() {
            std::fs::copy(&from, with_suffix(&copy, suffix)).expect("copy a sidecar");
            sidecars += 1;
        }
    }
    println!(
        "copied in {:.1}s with {sidecars} sidecars ({:.2} GB)",
        started.elapsed().as_secs_f64(),
        size_gb(&copy),
    );

    // And the part that matters: does it open, and is everything still in it?
    let at = rbl_db::LibraryLocation {
        master_db: copy.clone(),
        // Not the install any more — it is a file in a temporary directory,
        // and the read-only gate keys off that.
        is_real_install: false,
        ..location
    };
    let opened = Instant::now();
    match rbl_db::Library::open(at, rbl_db::OpenMode::ReadOnly) {
        Ok(db) => {
            let probe = db.schema();
            println!(
                "opened in {:.1}s: DBVersion {:?}, {} tables, support {:?}",
                opened.elapsed().as_secs_f64(),
                probe.db_version,
                probe.table_count,
                probe.support,
            );
            match rbl_index::load(&db) {
                Ok((library, stats)) => println!(
                    "read back {} tracks, {} playlists, {} memberships, {} histories",
                    library.len(),
                    stats.playlists,
                    stats.memberships,
                    stats.histories,
                ),
                Err(e) => println!("INDEX FAILED: {e}"),
            }
        }
        Err(e) => println!("OPEN FAILED: {e}"),
    }

    if keep {
        println!("kept at {}", copy.display());
    } else {
        let _ = std::fs::remove_dir_all(&into);
        println!("copy deleted");
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn size_gb(path: &Path) -> f64 {
    std::fs::metadata(path).map_or(0.0, |m| m.len() as f64 / 1_073_741_824.0)
}
