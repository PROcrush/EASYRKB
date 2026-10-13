//! What a track-id lookup costs, by scan and by map. READ-ONLY.
//!
//! `track_waveform` walked `ids` for every row a scroll went past. This is the
//! measurement that says what that cost against the real library, and what the
//! map already sitting behind `row_of` costs instead.
//!
//! `cargo run --release -p rbl-index --example idlookup`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open the library: {e}");
            return;
        }
    };
    let (library, _) = rbl_index::load(&db).expect("load");
    let n = library.ids.len();
    println!("{n} tracks");

    // A screenful, taken from the far end where a scan is worst — which is
    // also where a scroll spends most of its time in a big library.
    let sample: Vec<u64> = library.ids.iter().rev().take(40).copied().collect();

    // Warm the map, so what is timed is the lookup and not the one-off build.
    let _ = library.row_of_id(sample[0]);

    let start = Instant::now();
    for &id in &sample {
        std::hint::black_box(library.ids.iter().position(|&x| x == id));
    }
    let scan = start.elapsed();

    let start = Instant::now();
    for &id in &sample {
        std::hint::black_box(library.row_of_id(id));
    }
    let map = start.elapsed();

    println!(
        "a screenful of {} rows: scan {:>8.1} us, map {:>8.1} us  ({:.0}x)",
        sample.len(),
        scan.as_secs_f64() * 1e6,
        map.as_secs_f64() * 1e6,
        scan.as_secs_f64() / map.as_secs_f64().max(f64::MIN_POSITIVE),
    );
}
