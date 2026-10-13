//! READ-ONLY end-to-end: export real tracks from the installed library into a
//! temporary directory, then read the result back.
//!
//! Nothing is written outside the destination, and the library is opened
//! read-only. `cargo run --release -p rbl-export --example real -- [count] [dest]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn main() {
    // `nth(1)` and `nth(2)`: with `cargo run --example real -- 6 /dest` the
    // program sees [binary, "6", "/dest"]. Reading these one slot further
    // along made the count silently default to 20 and the destination to a
    // temporary directory, whatever was asked for.
    let count: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let dest = std::env::args()
        .nth(2)
        .map_or_else(|| std::env::temp_dir().join("rbl-export-demo"), PathBuf::from);

    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => { println!("cannot open library: {e}"); return; }
    };
    let share = db.location().share_root.clone();
    let (library, _) = rbl_index::load(&db).expect("index");

    // Pick analysed tracks whose audio is actually present.
    let mut tracks = Vec::new();
    let mut playlist_indices = Vec::new();
    for row in 0..library.len() as u32 {
        if tracks.len() >= count { break; }
        let i = row as usize;
        let path = library.folder_path.get(i);
        if path.is_empty() || !Path::new(path).exists() || library.bpm_x100[i] == 0 {
            continue;
        }

        // Copy the analysis rekordbox already produced.
        let mut analysis = Vec::new();
        let anlz_path = library.analysis_path.get(i);
        if !anlz_path.is_empty() {
            let dat = rbl_anlz::resolve(&share, anlz_path);
            for ext in ["DAT", "EXT", "2EX"] {
                let candidate = rbl_anlz::sibling(&dat, ext);
                if let Ok(bytes) = std::fs::read(&candidate) {
                    analysis.push((ext.to_owned(), bytes));
                }
            }
        }

        playlist_indices.push(tracks.len());
        tracks.push(rbl_export::SourceTrack {
            source_path: PathBuf::from(path),
            title: library.title.get(i).to_owned(),
            artist: library.artist_name(row).to_owned(),
            album: library.album_name(row).to_owned(),
            genre: library.genre_name(row).to_owned(),
            label: library.label_name(row).to_owned(),
            key: library.key_name(row).to_owned(),
            comment: library.comment.get(i).to_owned(),
            date_added: library.date_added.get(i).to_owned(),
            release_date: library.release_date.get(i).to_owned(),
            bpm_x100: library.bpm_x100[i],
            duration_sec: u16::try_from(library.length_sec[i]).unwrap_or(0),
            rating: library.rating[i],
            color_id: library.color[i],
            analysis,
            ..rbl_export::SourceTrack::default()
        });
    }

    if tracks.is_empty() {
        println!("no exportable tracks found");
        return;
    }

    let _ = std::fs::remove_dir_all(&dest);
    let playlists = vec![rbl_export::SourcePlaylist {
        name: "rbxport test".to_owned(),
        track_indices: playlist_indices,
        ..Default::default()
    }];

    let started = std::time::Instant::now();
    let report = match rbl_export::export(&dest, &tracks, &playlists) {
        Ok(r) => r,
        Err(e) => { println!("export failed: {e}"); return; }
    };
    let elapsed = started.elapsed();

    println!("exported to {}", dest.display());
    println!("  {} tracks, {} playlists, {} analysis files",
             report.tracks, report.playlists, report.analysis_files);
    println!("  {:.1} MB audio copied in {:.1}s ({:.0} MB/s)",
             report.bytes_copied as f64 / 1_048_576.0,
             elapsed.as_secs_f64(),
             report.bytes_copied as f64 / 1_048_576.0 / elapsed.as_secs_f64().max(0.001));
    println!("  export.pdb is {} bytes", report.pdb_bytes);
    if !report.skipped.is_empty() {
        println!("  skipped {} tracks", report.skipped.len());
    }

    let check = rbl_export::verify(&dest).expect("verify");
    println!("verified by re-reading:");
    println!("  parsed {}  tracks {}  playlists {}  entries {}",
             check.parsed, check.tracks, check.playlists, check.playlist_entries);
    println!("  audio present {} / {}   analysis present {}",
             check.audio_present, check.tracks, check.analysis_present);
    println!("  {}", if check.is_ok() { "OK" } else { "PROBLEMS FOUND" });
    for missing in check.missing_audio.iter().take(5) {
        println!("    missing: {missing}");
    }

    // Show a few rows the way a player would read them.
    let bytes = std::fs::read(dest.join("PIONEER/rekordbox/export.pdb")).unwrap();
    let pdb = rbl_pdb::Pdb::parse(&bytes).unwrap();
    if let Some(table) = pdb.table(rbl_pdb::PageType::Tracks) {
        println!("as a player would see it:");
        for row in pdb.track_rows(table).iter().take(5) {
            println!("  {:>3}  {:<40} {:>7.2} {:<4} {}",
                     row.id,
                     truncate(&row.title, 40),
                     f64::from(row.tempo_x100) / 100.0,
                     row.key_id,
                     row.file_path);
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_owned() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
