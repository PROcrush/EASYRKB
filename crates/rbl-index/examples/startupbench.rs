//! Read-only startup phases against the installed library; no app cache changes.
//! cargo run --release -p rbl-index --example startupbench
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
use std::time::Instant;
use rbl_db::{Library, OpenMode};
use rbl_index::cache::{self, Fingerprint};

fn fingerprint(db: &Library) -> Fingerprint {
    Fingerprint::of(&db.location().master_db,
        db.schema().db_version.and_then(|v| u32::try_from(v).ok()).unwrap_or(0),
        rbl_index::content_version(db).unwrap()).unwrap()
}

fn main() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("library.snapshot");
    for run in 1..=3 {
        let start = Instant::now();
        let db = Library::open_installed_read_only().unwrap();
        let open = start.elapsed();
        let fp = fingerprint(&db);
        let validation = start.elapsed() - open;
        let t = Instant::now();
        let reader = Library::open(db.location().clone(), OpenMode::ReadOnly).unwrap();
        let (library, stats) = rbl_index::load_with_cue_reader(&db, Some(reader)).unwrap();
        println!("run {run}: tracks={} open={}ms validation={}ms parallel-load={}ms read={}ms indexes={}ms total={}ms",
            library.len(), open.as_millis(), validation.as_millis(), t.elapsed().as_millis(),
            stats.read_ms, stats.index_ms, start.elapsed().as_millis());
        let t = Instant::now();
        let (serial, _) = rbl_index::load(&db).unwrap();
        println!("  single-reader load={}ms", t.elapsed().as_millis());
        assert_eq!(cache::encode(&library, fp), cache::encode(&serial, fp));
        cache::save(&path, &library, fp).unwrap();
        let t = Instant::now();
        let restored = cache::load(&path, fp).unwrap();
        println!("  snapshot={}ms size={}bytes", t.elapsed().as_millis(), std::fs::metadata(&path).unwrap().len());
        assert_eq!(cache::encode(&restored, fp), cache::encode(&library, fp));
        let t = Instant::now();
        std::thread::scope(|scope| {
            let pending = scope.spawn(|| cache::prepare(&path).unwrap());
            let live = Library::open_installed_read_only().unwrap();
            let restored = pending.join().unwrap().validated(fingerprint(&live)).unwrap();
            assert_eq!(restored.ids, library.ids);
        });
        println!("  overlapped warm backend={}ms", t.elapsed().as_millis());
    }
}
