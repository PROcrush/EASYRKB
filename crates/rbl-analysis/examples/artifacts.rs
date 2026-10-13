//! The analysis-artifacts test: what our analysis leaves on a track, and
//! whether rekordbox is still happy with it.
//!
//! One playlist — `RBX-BPM-MULTIBPM-RESULTS` by default, whose rows are the
//! multi-tempo rig's own copies — is stripped back to unanalysed, analysed
//! by the app itself (`scripts/analysis-artifacts/run.sh` drives the
//! compiled app; this rig does not analyse anything), and then checked for
//! the five things a track has to carry:
//!
//! | artifact | where it lives |
//! |---|---|
//! | key | `djmdContent.KeyID` → `djmdKey.ScaleName` |
//! | BPM | `djmdContent.BPM` |
//! | beat grid | `PQTZ` in the `.DAT` |
//! | preview waveform | `PWAV` and `PWV2` in the `.DAT`, `PWV4` in the `.EXT` |
//! | full overview waveform | `PWV3` and `PWV5` in the `.EXT`, `PWV6`/`PWV7` in the `.2EX` |
//!
//! `verify` is read-only and runs the same either side of rekordbox opening
//! the library, which is the whole point: the second run says whether
//! rekordbox kept what we wrote. `strip` **WRITES**, through the guarded
//! `Writer` (backup first, refused while rekordbox runs), and refuses
//! outright if any row in the playlist is not one of the rig's own copies.
//!
//! ```text
//! cargo run --release -p rbl-analysis --example artifacts -- strip  [playlist]
//! cargo run --release -p rbl-analysis --example artifacts -- verify [playlist]
//! ```
//!
//! `verify` exits 1 if anything is missing, and prints one line per track
//! per artifact so a failure says which track and which artifact.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use rbl_db::write::Writer;
use rusqlite::params;

/// The playlist the test runs on unless one is named.
const DEFAULT_PLAYLIST: &str = "RBX-BPM-MULTIBPM-RESULTS";

/// Where the multi-tempo rig keeps the audio it imported. Only rows whose
/// file is under here may be stripped: everything in the default playlist
/// is a copy this project made, and nothing in the real collection should
/// ever lose its analysis to a test.
fn staging_dir() -> PathBuf {
    let raw = std::env::var("RB_LITE_MULTIBPM").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/multibpm"),
        PathBuf::from,
    );
    raw.canonicalize().unwrap_or(raw).join("tracks")
}

/// A track as the library has it, with everything the checks need.
#[derive(Debug, Clone)]
struct Track {
    id: String,
    title: String,
    path: PathBuf,
    bpm_x100: u32,
    key: String,
    analysis_path: String,
    analysed: i64,
    image_path: String,
    content_link: Option<i64>,
    analysis_updated: String,
}

impl Track {
    fn is_staged(&self) -> bool {
        normalized(&self.path).starts_with(staging_dir())
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "verify".to_owned());
    let playlist = std::env::args().nth(2).unwrap_or_else(|| DEFAULT_PLAYLIST.to_owned());
    let outcome = match mode.as_str() {
        "strip" => strip(&playlist),
        "verify" => verify(&playlist),
        other => Err(format!("unknown mode {other:?}; see the top of artifacts.rs")),
    };
    if let Err(e) = outcome {
        println!("artifacts {mode}: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------- library

fn members(conn: &rusqlite::Connection, playlist: &str) -> Result<Vec<Track>, String> {
    let id: String = conn
        .query_row(
            "SELECT ID FROM djmdPlaylist WHERE Name = ?1 AND rb_local_deleted = 0",
            params![playlist],
            |r| r.get(0),
        )
        .map_err(|_| format!("no playlist named {playlist:?}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT c.ID, COALESCE(c.Title, ''), COALESCE(c.FolderPath, ''), COALESCE(c.BPM, 0),
                    COALESCE(k.ScaleName, ''), COALESCE(c.AnalysisDataPath, ''), COALESCE(c.Analysed, 0),
                    COALESCE(c.ImagePath, ''), c.ContentLink, COALESCE(c.AnalysisUpdated, '')
             FROM djmdSongPlaylist sp
             JOIN djmdContent c ON c.ID = sp.ContentID
             LEFT JOIN djmdKey k ON k.ID = c.KeyID
             WHERE sp.PlaylistID = ?1 AND sp.rb_local_deleted = 0 AND c.rb_local_deleted = 0
             ORDER BY sp.TrackNo",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<Track> = stmt
        .query_map(params![id], |r| {
            Ok(Track {
                id: r.get(0)?,
                title: r.get(1)?,
                path: PathBuf::from(r.get::<_, String>(2)?),
                bpm_x100: u32::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                key: r.get(4)?,
                analysis_path: r.get(5)?,
                analysed: r.get(6)?,
                image_path: r.get(7)?,
                content_link: r.get(8)?,
                analysis_updated: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    if rows.is_empty() {
        return Err(format!("{playlist} has no tracks"));
    }
    Ok(rows)
}

// ---------------------------------------------------------------- strip

fn strip(playlist: &str) -> Result<(), String> {
    let location = rbl_db::detect().map_err(|e| format!("no library: {e}"))?;
    let backups = staging_dir().with_file_name("backups");
    let mut writer = Writer::open(location, backups)
        .map_err(|e| format!("cannot open the library for writing: {e}"))?;
    let share = writer.library().location().share_root.clone();
    let tracks = members(writer.library().connection(), playlist)?;

    // Refused rather than skipped: a run that silently left a track
    // analysed would go on to verify it and call the test passed.
    let outside: Vec<&Track> = tracks.iter().filter(|t| !t.is_staged()).collect();
    if !outside.is_empty() {
        for t in &outside {
            println!("  NOT OURS  {}  ({})", t.title, t.path.display());
        }
        return Err(format!(
            "{} of {} tracks in {playlist} are not under {}; this rig only strips its own copies",
            outside.len(),
            tracks.len(),
            staging_dir().display()
        ));
    }

    println!("stripping {} tracks in {playlist}", tracks.len());
    for track in &tracks {
        // The row first, then the files: a row still naming files that are
        // gone reads as analysed with nothing to draw, and that is the one
        // state a crash here must not leave behind.
        writer.clear_analysis(&track.id).map_err(|e| format!("clear {}: {e}", track.title))?;
        let mut gone = Vec::new();
        if !track.analysis_path.is_empty() {
            let dat = rbl_anlz::resolve(&share, &track.analysis_path);
            for ext in ["DAT", "EXT", "2EX", "3EX"] {
                let file = rbl_anlz::sibling(&dat, ext);
                if std::fs::remove_file(&file).is_ok() {
                    gone.push(ext);
                }
            }
            // The folder is one track's own, and rekordbox leaves none empty.
            if let Some(dir) = dat.parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }
        println!("  stripped  {:<58}  files removed: {}", truncate(&track.title, 58), if gone.is_empty() { "none".to_owned() } else { gone.join(" ") });
    }
    println!("strip: {} tracks in {playlist} are unanalysed", tracks.len());
    Ok(())
}

// ---------------------------------------------------------------- verify

/// One artifact's verdict.
struct Check {
    what: &'static str,
    ok: bool,
    detail: String,
}

fn check(what: &'static str, ok: bool, detail: impl Into<String>) -> Check {
    Check { what, ok, detail: detail.into() }
}

/// Whether a waveform section is there and holds something to draw.
///
/// Check framing as well as payload. Nonzero data behind an incompatible
/// header passed the old test while rekordbox drew a flat preview.
fn waveform(file: Option<&rbl_anlz::Anlz>, tag: &[u8; 4]) -> (bool, String) {
    let name = String::from_utf8_lossy(tag).into_owned();
    let Some(anlz) = file else { return (false, format!("{name}: no file")) };
    let Some((_, bytes)) = anlz.waveform(tag) else { return (false, format!("{name}: missing")) };
    if bytes.is_empty() {
        return (false, format!("{name}: empty"));
    }
    if bytes.iter().all(|&b| b == 0) {
        return (false, format!("{name}: {} bytes, all zero", bytes.len()));
    }
    let section = anlz.sections.iter().find(|s| &s.tag.0 == tag).unwrap();
    if tag == b"PWV6" && (section.header != [0, 0, 0, 3, 0, 0, 4, 176] || bytes.len() != 3600) {
        return (false, "PWV6: expected 20-byte header, stride 3, count 1200".to_owned());
    }
    (true, format!("{name} {}B", bytes.len()))
}

/// Joins several waveform verdicts into one line for an artifact.
fn waveforms(parts: &[(bool, String)]) -> (bool, String) {
    let ok = parts.iter().all(|(ok, _)| *ok);
    let detail = parts.iter().map(|(_, d)| d.as_str()).collect::<Vec<_>>().join(", ");
    (ok, detail)
}

fn verify(playlist: &str) -> Result<(), String> {
    let db = rbl_db::Library::open_installed_read_only().map_err(|e| format!("cannot open library: {e}"))?;
    let share = db.location().share_root.clone();
    let tracks = members(db.connection(), playlist)?;
    println!("verifying {} tracks in {playlist}", tracks.len());
    println!();

    let mut failed = 0;
    for track in &tracks {
        let dat_path = (!track.analysis_path.is_empty()).then(|| rbl_anlz::resolve(&share, &track.analysis_path));
        let dat = dat_path.as_ref().and_then(|p| rbl_anlz::Anlz::read(p).ok());
        let ext = dat_path.as_ref().and_then(|p| rbl_anlz::Anlz::read(&rbl_anlz::sibling(p, "EXT")).ok());
        let two_ex = dat_path.as_ref().and_then(|p| rbl_anlz::Anlz::read(&rbl_anlz::sibling(p, "2EX")).ok());

        let grid = dat.as_ref().and_then(rbl_anlz::Anlz::beat_grid).unwrap_or_default();
        let ascending = grid.windows(2).all(|w| w[1].time_ms > w[0].time_ms);
        let numbered = grid.iter().all(|b| (1..=4).contains(&b.beat_number));
        let embedded = rbl_db::import::read_artwork(&track.path).map_err(|e| e.to_string())?.is_some();
        let artwork = rbl_anlz::resolve(&share, &track.image_path);
        let artwork_ok = if track.image_path.is_empty() {
            !embedded
        } else {
            artwork.is_file() && artwork.with_file_name("artwork_s.jpg").is_file()
                && artwork.with_file_name("artwork_m.jpg").is_file()
        };

        let checks = [
            check("bpm", track.bpm_x100 > 0, format!("{:.2}", f64::from(track.bpm_x100) / 100.0)),
            check("key", !track.key.is_empty(), track.key.clone()),
            check("artwork", artwork_ok, if embedded { "embedded cover and thumbnails" } else { "no embedded cover required" }),
            check(
                "beat grid",
                grid.len() >= 2 && ascending && numbered,
                if grid.is_empty() {
                    "no PQTZ".to_owned()
                } else {
                    format!(
                        "PQTZ {} beats, {:.2}–{:.2} BPM, {}{}",
                        grid.len(),
                        f64::from(grid.iter().map(|b| b.tempo_x100).min().unwrap_or(0)) / 100.0,
                        f64::from(grid.iter().map(|b| b.tempo_x100).max().unwrap_or(0)) / 100.0,
                        if ascending { "ascending" } else { "OUT OF ORDER" },
                        if numbered { "" } else { ", BAD BEAT NUMBERS" },
                    )
                },
            ),
            {
                // What the browser row and the strip above the deck draw.
                let (ok, detail) = waveforms(&[
                    waveform(dat.as_ref(), b"PWAV"),
                    waveform(dat.as_ref(), b"PWV2"),
                    waveform(ext.as_ref(), b"PWV4"),
                ]);
                check("preview waveform", ok, detail)
            },
            {
                // The whole-track colour overview and the scrolling waveforms.
                let (ok, detail) = waveforms(&[
                    waveform(ext.as_ref(), b"PWV3"),
                    waveform(ext.as_ref(), b"PWV5"),
                    waveform(two_ex.as_ref(), b"PWV6"),
                    waveform(two_ex.as_ref(), b"PWV7"),
                ]);
                check("full waveform", ok, detail)
            },
            check(
                "row",
                track.analysed != 0 && !track.analysis_path.is_empty() && track.content_link.is_some()
                    && track.analysis_updated.parse::<u64>().is_ok(),
                format!("Analysed {}, {}", track.analysed, if track.analysis_path.is_empty() { "no path" } else { &track.analysis_path }),
            ),
        ];

        let bad = checks.iter().filter(|c| !c.ok).count();
        failed += usize::from(bad > 0);
        println!("{}", track.title);
        for c in &checks {
            println!("  {:<16} {}  {}", c.what, if c.ok { "ok  " } else { "MISS" }, c.detail);
        }
        println!();
    }

    let n = tracks.len();
    println!("verify: {} of {n} tracks carry every artifact", n - failed);
    if failed > 0 {
        return Err(format!("{failed} of {n} tracks are missing something"));
    }
    Ok(())
}

// ---------------------------------------------------------------- plumbing

/// `.` and `..` resolved lexically, as the importer stores paths.
fn normalized(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_owned() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
