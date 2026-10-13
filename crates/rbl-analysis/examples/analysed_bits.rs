//! Does `djmdContent.Analysed` record which analysis tags a track has?
//! READ-ONLY.
//!
//! The field is a bitfield with 105, 1, 17, 74 and 106 observed, and nothing
//! explains it. This tests one hypothesis directly: sample tracks at each
//! value, parse their ANLZ, and report which tags are present. If the bits
//! track tag presence, tracks sharing a value share a tag set.
//!
//! `cargo run --release -p rbl-analysis --example analysed_bits`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::Path;

/// Tracks to sample per distinct `Analysed` value.
const PER_VALUE: usize = 12;

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let share = db.location().share_root.clone();

    // Straight from the database: the index stores Analysed as a flag, and
    // this needs the raw bitfield.
    let mut stmt = db
        .connection()
        .prepare(
            "SELECT Analysed, COALESCE(AnalysisDataPath, '') FROM djmdContent \
             WHERE rb_local_deleted = 0 ORDER BY Analysed",
        )
        .expect("query");
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0).unwrap_or(0), r.get::<_, String>(1)?)))
        .expect("rows")
        .filter_map(Result::ok)
        .collect();

    let mut by_value: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    let mut counted: BTreeMap<i64, usize> = BTreeMap::new();

    for (value, relative) in rows {
        let seen = counted.entry(value).or_default();
        if *seen >= PER_VALUE {
            continue;
        }
        let relative = relative.as_str();
        if relative.is_empty() {
            by_value.entry(value).or_default().push("(no analysis path)".to_owned());
            *seen += 1;
            continue;
        }
        let path = share.join(relative.trim_start_matches(['/', '\\']));
        let Ok(bytes) = std::fs::read(&path) else {
            by_value.entry(value).or_default().push("(file missing)".to_owned());
            *seen += 1;
            continue;
        };
        let Ok(file) = rbl_anlz::parse(&bytes) else {
            by_value.entry(value).or_default().push("(unparseable)".to_owned());
            *seen += 1;
            continue;
        };
        let mut tags: Vec<String> =
            file.sections.iter().map(|s| s.tag.as_str().to_owned()).collect();
        tags.sort();
        tags.dedup();
        by_value.entry(value).or_default().push(tags.join(" "));
        *seen += 1;

        // Also read the extended file beside it, where the colour waveforms
        // live: a bit may track those rather than the plain ones.
        let ext = path.with_extension("EXT");
        if let Ok(ext_bytes) = std::fs::read(&ext) {
            if let Ok(ext_file) = rbl_anlz::parse(&ext_bytes) {
                let mut ext_tags: Vec<String> =
                    ext_file.sections.iter().map(|s| s.tag.as_str().to_owned()).collect();
                ext_tags.sort();
                ext_tags.dedup();
                if let Some(last) = by_value.get_mut(&value).and_then(|v| v.last_mut()) {
                    last.push_str("  |EXT| ");
                    last.push_str(&ext_tags.join(" "));
                }
            }
        }
    }

    for (value, samples) in &by_value {
        println!("== Analysed = {value} ({value:#012b}), {} sampled ==",
                 counted.get(value).copied().unwrap_or(0));
        let mut distinct: Vec<&String> = samples.iter().collect();
        distinct.sort();
        distinct.dedup();
        for set in distinct.iter().take(4) {
            let count = samples.iter().filter(|s| s == set).count();
            println!("  {count:>2}x  {set}");
        }
        if distinct.len() > 4 {
            println!("  … {} more distinct sets", distinct.len() - 4);
        }
        println!();
    }
    let _ = Path::new("");
}
