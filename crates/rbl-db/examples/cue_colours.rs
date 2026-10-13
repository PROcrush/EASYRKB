//! What RGB a `djmdCue.ColorTableIndex` means. READ-ONLY, both sides.
//!
//! The index was the last `[UNKNOWN]` in the cue writer: the library stores a
//! number and rekordbox paints a colour, and nothing on this machine had the
//! two together — not `djmdColor`, not the skins, not the share-tree analysis
//! files, whose cue lists are empty, and not `product.db`.
//!
//! An **export** has them together. When rekordbox writes a stick it puts the
//! cues in the `PCO2` section of each `ANLZ*.EXT`, and each entry carries the
//! index *and* the RGB for it. So a stick that rekordbox exported is a reading
//! of the palette, without an edit to the library and without a diff recording.
//!
//! This reads one, joins it back to `djmdCue` by filename to check that the
//! export's `color_code` really is `ColorTableIndex`, and prints the table.
//!
//! `cargo run -p rbl-db --example cue_colours -- /Volumes/SD`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A hot cue as the export drew it: slot, `ColorTableIndex`, RGB.
type DrawnCue = (u32, u8, [u8; 3]);

fn main() {
    let root = std::env::args().nth(1).map_or_else(
        || PathBuf::from("/Volumes/SD"),
        PathBuf::from,
    );
    let anlz = root.join(".PIONEER").join("USBANLZ");
    if !anlz.is_dir() {
        println!("no export at {}", anlz.display());
        println!("point this at a stick rekordbox exported: --example cue_colours -- /Volumes/NAME");
        return;
    }

    // filename -> the hot cues the export drew, as (slot, index, rgb).
    let mut exported: BTreeMap<String, Vec<DrawnCue>> = BTreeMap::new();
    // index -> every RGB seen for it. More than one would mean the index is
    // not a palette at all.
    let mut palette: BTreeMap<u8, BTreeSet<[u8; 3]>> = BTreeMap::new();

    for ext in ext_files(&anlz) {
        let Ok(file) = rbl_anlz::Anlz::read(&ext) else { continue };
        let Some(path) = file.path() else { continue };
        let Some(name) = Path::new(&path).file_name().and_then(|n| n.to_str()) else { continue };
        for entry in file.cue_entries() {
            let (Some(code), Some(rgb)) = (entry.color_code, entry.rgb) else { continue };
            if entry.hot_cue == 0 {
                continue; // memory cues colour themselves through `color_id`.
            }
            palette.entry(code).or_default().insert(rgb);
            exported.entry(name.to_owned()).or_default().push((entry.hot_cue, code, rgb));
        }
    }

    println!("{} exported tracks with coloured hot cues", exported.len());

    println!("\nColorTableIndex -> RGB, as rekordbox wrote it");
    for (code, seen) in &palette {
        let shades: Vec<String> =
            seen.iter().map(|c| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])).collect();
        let note = if seen.len() > 1 { "  <- not one colour, so not a palette" } else { "" };
        println!("  {code:>3}  {}{note}", shades.join(" "));
    }

    match rbl_db::Library::open_installed_read_only() {
        Ok(db) => {
            check_against_library(&db, &exported);
            still_unread(&db, &palette);
        }
        Err(e) => println!("\ncannot open the library to cross-check: {e}"),
    }
}

/// Which indices the library uses that this export did not carry, and one
/// track for each — so the stick that reads the rest of the palette can be
/// exported deliberately rather than hopefully.
fn still_unread(db: &rbl_db::Library, palette: &BTreeMap<u8, BTreeSet<[u8; 3]>>) {
    let conn = db.connection();
    let Ok(mut stmt) = conn.prepare(
        "SELECT c.ColorTableIndex, COUNT(*), MIN(t.Title) FROM djmdCue c \
         JOIN djmdContent t ON t.ID = c.ContentID \
         WHERE c.rb_local_deleted = 0 AND c.Kind <> 0 AND c.ColorTableIndex > 0 \
         GROUP BY c.ColorTableIndex ORDER BY COUNT(*) DESC",
    ) else {
        return;
    };
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?))
    });
    let Ok(rows) = rows else { return };

    let mut unread = Vec::new();
    for (index, cues, title) in rows.filter_map(Result::ok) {
        if u8::try_from(index).is_ok_and(|i| palette.contains_key(&i)) {
            continue;
        }
        unread.push((index, cues, title.unwrap_or_default()));
    }
    if unread.is_empty() {
        println!("\nevery index the library uses has an RGB — the palette is complete");
        return;
    }
    println!("\nstill unread, with a track that would carry each into an export:");
    for (index, cues, title) in unread {
        println!("  {index:>3}  {cues:>6} cues  {title}");
    }
}

/// The claim under test: the export's `color_code` is the same number the
/// library stores as `ColorTableIndex` for the same cue.
fn check_against_library(
    db: &rbl_db::Library,
    exported: &BTreeMap<String, Vec<DrawnCue>>,
) {
    let conn = db.connection();
    let Ok(mut stmt) = conn.prepare(
        "SELECT c.ColorTableIndex FROM djmdCue c \
         JOIN djmdContent t ON t.ID = c.ContentID \
         WHERE c.rb_local_deleted = 0 AND c.Kind <> 0 AND t.FileNameL = ?1 \
         ORDER BY c.InMsec",
    ) else {
        println!("\ncannot prepare the cross-check");
        return;
    };

    let (mut matched, mut disagreed, mut absent) = (0usize, 0usize, 0usize);
    for (name, cues) in exported {
        let stored: Vec<i64> = match stmt.query_map([name], |r| r.get::<_, Option<i64>>(0)) {
            Ok(rows) => rows.filter_map(Result::ok).flatten().collect(),
            Err(_) => continue,
        };
        if stored.is_empty() {
            absent += 1;
            continue;
        }
        // The export drops duplicates the library keeps, so compare the sets
        // of indices rather than position by position.
        let from_export: BTreeSet<i64> = cues.iter().map(|&(_, code, _)| i64::from(code)).collect();
        let from_library: BTreeSet<i64> = stored.into_iter().collect();
        if from_export.is_subset(&from_library) {
            matched += 1;
        } else {
            disagreed += 1;
            if disagreed <= 5 {
                println!("  {name}: export {from_export:?} library {from_library:?}");
            }
        }
    }
    println!(
        "\ncross-check against djmdCue: {matched} tracks agree, {disagreed} disagree, \
         {absent} not found by filename"
    );
}

fn ext_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("EXT")) {
                out.push(path);
            }
        }
    }
    out
}
