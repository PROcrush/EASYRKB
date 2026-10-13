//! Compare full and incremental refreshes against the installed library, READ-ONLY.
//! Only the in-memory copy is changed; no edits are sent to the database.
//! cargo run -p rbl-index --example metadata_refresh
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
use std::time::Instant;

fn main() {
    let db = rbl_db::Library::open_installed_read_only().expect("read-only library");
    let (original, _) = rbl_index::load(&db).expect("index");
    let Some(id) = original.ids.first() else { return };
    for _ in 0..3 {
        let mut before = original.clone();
        before.rating[0] = (before.rating[0] + 1) % 6;
        let mut comments = rbl_index::strings::StrColumn::default();
        for row in 0..before.len() {
            comments.push(if row == 0 { "temporary in-memory comment" } else { before.comment.get(row) });
        }
        before.comment = comments;
        let started = Instant::now();
        let mut next = before.clone();
        rbl_index::reload_metadata(&db, &mut next, &[id.to_string()]).expect("metadata");
        println!("metadata refresh including snapshot copy: {} us", started.elapsed().as_micros());
        let started = Instant::now();
        rbl_index::reload_histories(&db, &next).expect("history");
        println!("history refresh: {} us", started.elapsed().as_micros());
        let started = Instant::now();
        rbl_index::load(&db).expect("full reload");
        println!("full library reload: {} us", started.elapsed().as_micros());
    }
}
