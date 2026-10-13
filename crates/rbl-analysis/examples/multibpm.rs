//! The multi-tempo test: our beat and key analysis on the playlist
//! `RBX-BPM-MULTIBPM-TEST`, judged inside rekordbox itself.
//!
//! Copies of the playlist's files are imported into
//! `RBX-BPM-MULTIBPM-RESULTS`, analysed by us, and registered in the
//! library — analysis files in rekordbox's own shape under the share tree,
//! BPM, key and `Analysed` on the row — so a copy can be loaded on a deck
//! next to its original and the two grids compared by eye. `report` scores
//! the copies against the originals the way the golden gate does.
//!
//! **WRITES to the library**, through the guarded `Writer` (backup first,
//! refused while rekordbox is running) and only on rows this rig imported
//! itself: their files live under the staging directory, and nothing
//! outside it is deleted or re-registered.
//!
//! ```text
//! cargo run --release -p rbl-analysis --example multibpm -- run [filter]
//! cargo run --release -p rbl-analysis --example multibpm -- <mode> [filter]
//! ```
//!
//! | mode      | does |
//! |-----------|------|
//! | `stage`   | copies the originals' files into the staging directory |
//! | `reset`   | removes the previous run's copies from the collection, and their analysis files |
//! | `import`  | imports the staged copies and puts them in the RESULTS playlist |
//! | `analyse` | analyses each copy, writes its `.DAT`/`.EXT`, registers it |
//! | `report`  | scores each copy's registered grid, BPM and key against its original |
//! | `check`   | says whether rekordbox has since touched anything we registered |
//! | `run`     | stage, reset, import, analyse, report |
//!
//! `filter` is a case-insensitive substring of the original's title or file
//! name, to run one song. The staging directory is `test-mp3s/tracks`
//! (gitignored, outside `target/` so a clean build cannot delete files a
//! real library row still points at) or `RB_LITE_MULTIBPM`; backups go
//! beside it.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rbl_analysis::tempo::Beat;
use rbl_db::write::{AnalysisRegistration, TrackField, Writer};
use rusqlite::params;

#[path = "common/score.rs"]
mod score;
use score::{grid_match, offsets, runs_line, BPM_TOLERANCE, GRID_PASS, MS_TOLERANCE};

const TEST_PLAYLIST: &str = "RBX-BPM-MULTIBPM-TEST";
const RESULTS_PLAYLIST: &str = "RBX-BPM-MULTIBPM-RESULTS";

/// A track as the library has it.
#[derive(Debug, Clone)]
struct Track {
    id: String,
    title: String,
    path: PathBuf,
    file_name: String,
    bpm_x100: u32,
    key: String,
    analysis_path: String,
    analysed: Option<i64>,
    updated_at: String,
}

impl Track {
    /// Whether the row's file is one of ours. Compared lexically cleaned:
    /// rows from before the rig canonicalised its paths carry a `..`.
    fn is_staged(&self) -> bool {
        normalized(&self.path).starts_with(staging_dir())
    }

    fn matches(&self, filter: Option<&str>) -> bool {
        filter.is_none_or(|f| {
            self.title.to_lowercase().contains(f) || self.file_name.to_lowercase().contains(f)
        })
    }
}

/// Canonical, because the path is what the library rows will carry and
/// rekordbox marks a row whose path holds `..` as missing.
fn root() -> PathBuf {
    let raw = std::env::var("RB_LITE_MULTIBPM").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-mp3s"),
        PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&raw);
    raw.canonicalize().unwrap_or(raw)
}
fn staging_dir() -> PathBuf {
    root().join("tracks")
}
fn registered_file() -> PathBuf {
    root().join("registered.tsv")
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "run".to_owned());
    let filter = std::env::args().nth(2).map(|f| f.to_lowercase());
    let filter = filter.as_deref();
    let outcome = match mode.as_str() {
        "stage" => stage(filter),
        "reset" => with_writer(|w| reset(w, filter)),
        "import" => with_writer(|w| import(w, filter)),
        "analyse" | "analyze" => with_writer(|w| analyse(w, filter)),
        "report" => report(filter),
        "check" => check(filter),
        "run" => stage(filter).and_then(|()| {
            with_writer(|w| {
                reset(w, filter)?;
                import(w, filter)?;
                analyse(w, filter)
            })
        }).and_then(|()| report(filter)),
        other => Err(format!("unknown mode {other:?}; see the top of multibpm.rs")),
    };
    if let Err(e) = outcome {
        println!("multibpm {mode}: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------- library

fn open_read_only() -> Result<rbl_db::Library, String> {
    rbl_db::Library::open_installed_read_only().map_err(|e| format!("cannot open library: {e}"))
}

/// One writer for the whole run: one backup, one process gate per action.
fn with_writer(f: impl FnOnce(&mut Writer) -> Result<(), String>) -> Result<(), String> {
    let location = rbl_db::detect().map_err(|e| format!("no library: {e}"))?;
    let mut writer = Writer::open(location, root().join("backups"))
        .map_err(|e| format!("cannot open the library for writing: {e}"))?;
    f(&mut writer)
}

fn playlist_id(conn: &rusqlite::Connection, name: &str) -> Result<String, String> {
    conn.query_row(
        "SELECT ID FROM djmdPlaylist WHERE Name = ?1 AND rb_local_deleted = 0",
        params![name],
        |r| r.get(0),
    )
    .map_err(|_| format!("no playlist named {name:?}"))
}

fn members(conn: &rusqlite::Connection, playlist: &str) -> Result<Vec<Track>, String> {
    let id = playlist_id(conn, playlist)?;
    let mut stmt = conn
        .prepare(
            "SELECT c.ID, COALESCE(c.Title, ''), COALESCE(c.FolderPath, ''), COALESCE(c.FileNameL, ''),
                    COALESCE(c.BPM, 0), COALESCE(k.ScaleName, ''), COALESCE(c.AnalysisDataPath, ''),
                    c.Analysed, c.updated_at
             FROM djmdSongPlaylist sp
             JOIN djmdContent c ON c.ID = sp.ContentID
             LEFT JOIN djmdKey k ON k.ID = c.KeyID
             WHERE sp.PlaylistID = ?1 AND sp.rb_local_deleted = 0 AND c.rb_local_deleted = 0
             ORDER BY sp.TrackNo",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![id], |r| {
            Ok(Track {
                id: r.get(0)?,
                title: r.get(1)?,
                path: PathBuf::from(r.get::<_, String>(2)?),
                file_name: r.get(3)?,
                bpm_x100: u32::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
                key: r.get(5)?,
                analysis_path: r.get(6)?,
                analysed: r.get(7)?,
                updated_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(rows)
}

fn originals(conn: &rusqlite::Connection, filter: Option<&str>) -> Result<Vec<Track>, String> {
    let all = members(conn, TEST_PLAYLIST)?;
    let picked: Vec<Track> = all.into_iter().filter(|t| t.matches(filter)).collect();
    if picked.is_empty() {
        return Err(format!("nothing in {TEST_PLAYLIST} matches {filter:?}"));
    }
    Ok(picked)
}

/// The staged copy's name: the original's file name, unless two originals
/// share one, when the content id is prefixed.
fn staged_name(track: &Track, all: &[Track]) -> String {
    let shared = all.iter().filter(|t| t.file_name == track.file_name).count() > 1;
    if shared { format!("{}_{}", track.id, track.file_name) } else { track.file_name.clone() }
}

/// A copy in the RESULTS playlist paired with its original, by file name.
fn pairs(conn: &rusqlite::Connection, filter: Option<&str>) -> Result<Vec<(Track, Track)>, String> {
    let all = members(conn, TEST_PLAYLIST)?;
    let copies = members(conn, RESULTS_PLAYLIST)?;
    let staging = staging_dir();
    let mut out = Vec::new();
    for copy in copies {
        if !copy.is_staged() {
            println!("  skipping {} — not under {}", copy.title, staging.display());
            continue;
        }
        let Some(original) = all.iter().find(|t| staged_name(t, &all) == copy.file_name) else {
            println!("  skipping {} — no original with that file name in {TEST_PLAYLIST}", copy.title);
            continue;
        };
        if original.matches(filter) || copy.matches(filter) {
            out.push((copy, original.clone()));
        }
    }
    if out.is_empty() {
        return Err(format!("no copies in {RESULTS_PLAYLIST} match {filter:?}; run `import` first"));
    }
    Ok(out)
}

// ---------------------------------------------------------------- stage

fn stage(filter: Option<&str>) -> Result<(), String> {
    let db = open_read_only()?;
    let all = members(db.connection(), TEST_PLAYLIST)?;
    let picked = originals(db.connection(), filter)?;
    let dir = staging_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    println!("staging {} of {} tracks into {}", picked.len(), all.len(), dir.display());
    for track in &picked {
        let target = dir.join(staged_name(track, &all));
        let source_len = std::fs::metadata(&track.path).map(|m| m.len()).map_err(|e| format!("{}: {e}", track.path.display()))?;
        if std::fs::metadata(&target).map(|m| m.len()).ok() == Some(source_len) {
            println!("  kept    {}", target.file_name().unwrap().to_string_lossy());
            continue;
        }
        std::fs::copy(&track.path, &target).map_err(|e| format!("copy {} -> {}: {e}", track.path.display(), target.display()))?;
        println!("  copied  {}  ({:.1} MB)", target.file_name().unwrap().to_string_lossy(), source_len as f64 / 1e6);
    }
    Ok(())
}

// ---------------------------------------------------------------- reset

fn reset(writer: &mut Writer, filter: Option<&str>) -> Result<(), String> {
    let staging = staging_dir();
    let share = writer.library().location().share_root.clone();
    let copies = members(writer.library().connection(), RESULTS_PLAYLIST)?;
    let mut registered = read_registered();
    let mut removed = 0;
    for copy in copies.iter().filter(|c| c.matches(filter)) {
        if !copy.is_staged() {
            println!("  leaving {} — not under {}", copy.title, staging.display());
            continue;
        }
        writer.delete_track(&copy.id).map_err(|e| format!("delete {}: {e}", copy.title))?;
        registered.remove(&copy.id);
        if !copy.analysis_path.is_empty() {
            let dat = rbl_anlz::resolve(&share, &copy.analysis_path);
            if let Some(dir) = dat.parent() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
        removed += 1;
        println!("  removed {}", copy.title);
    }
    write_registered(&registered)?;
    println!("reset: {removed} copies removed from the collection");
    Ok(())
}

// ---------------------------------------------------------------- import

fn import(writer: &mut Writer, filter: Option<&str>) -> Result<(), String> {
    let all = members(writer.library().connection(), TEST_PLAYLIST)?;
    let picked = originals(writer.library().connection(), filter)?;
    let results = playlist_id(writer.library().connection(), RESULTS_PLAYLIST)?;
    let dir = staging_dir();
    let mut ids = Vec::new();
    for track in &picked {
        let file = dir.join(staged_name(track, &all));
        if !file.exists() {
            return Err(format!("{} is not staged; run `stage` first", file.display()));
        }
        let id = match writer.import_file(&file) {
            Ok(id) => id,
            Err(rbl_db::DbError::WriteRefused(reason)) => {
                println!("  skipped {}: {reason}", track.title);
                continue;
            }
            Err(e) => return Err(format!("import {}: {e}", file.display())),
        };
        // The original's title as rekordbox shows it, so the two rows read
        // alike in the browser; a WAV has no tag to take it from.
        writer.set_field(&id, TrackField::Title, &track.title).map_err(|e| e.to_string())?;
        println!("  imported {}  as {id}", track.title);
        ids.push(id);
    }
    if !ids.is_empty() {
        writer.add_tracks(&results, &ids).map_err(|e| format!("add to {RESULTS_PLAYLIST}: {e}"))?;
    }
    println!("import: {} copies in {RESULTS_PLAYLIST}", ids.len());
    Ok(())
}

// ---------------------------------------------------------------- analyse

/// One row of `registered.tsv`: what we wrote, to tell later whether
/// rekordbox rewrote it.
struct Registered {
    id: String,
    title: String,
    path: String,
    bpm_x100: u32,
    key: String,
    dat_hash: u64,
    ext_hash: u64,
}

fn analyse(writer: &mut Writer, filter: Option<&str>) -> Result<(), String> {
    let share = writer.library().location().share_root.clone();
    let pairs = pairs(writer.library().connection(), filter)?;
    let mut registered = read_registered();
    for (copy, original) in &pairs {
        let started = Instant::now();
        let audio = rbl_audio::decode_mono(&copy.path, None).map_err(|e| format!("decode {}: {e}", copy.path.display()))?;
        let analysis = rbl_analysis::analyse(&audio.samples, audio.sample_rate);
        let (dat, ext) = author(&copy.file_name, &analysis);

        let relative = writer.analysis_data_path_for(&copy.id).map_err(|e| e.to_string())?;
        let dat_path = rbl_anlz::resolve(&share, &relative);
        let ext_path = rbl_anlz::sibling(&dat_path, "EXT");
        std::fs::create_dir_all(dat_path.parent().unwrap()).map_err(|e| format!("{}: {e}", dat_path.display()))?;
        std::fs::write(&dat_path, &dat).map_err(|e| format!("{}: {e}", dat_path.display()))?;
        std::fs::write(&ext_path, &ext).map_err(|e| format!("{}: {e}", ext_path.display()))?;

        let bpm_x100 = (analysis.tempo.bpm * 100.0).round().clamp(0.0, f64::from(u32::MAX)) as u32;
        let key = analysis.key.as_ref().map(|k| k.name.clone()).unwrap_or_default();
        writer
            .register_analysis(
                &copy.id,
                &AnalysisRegistration { bpm_x100, key: (!key.is_empty()).then_some(key.as_str()), analysis_data_path: &relative },
            )
            .map_err(|e| format!("register {}: {e}", copy.title))?;

        println!(
            "  {:<58} {:>7.2} BPM  {:<4} {:>4} beats  {} segments  {:.1}s   (rb {:.2} {})",
            truncate(&copy.title, 58),
            analysis.tempo.bpm,
            key,
            analysis.tempo.beats.len(),
            analysis.tempo.segments.len(),
            started.elapsed().as_secs_f64(),
            f64::from(original.bpm_x100) / 100.0,
            original.key,
        );
        registered.insert(
            copy.id.clone(),
            Registered { id: copy.id.clone(), title: copy.title.clone(), path: relative, bpm_x100, key, dat_hash: fnv(&dat), ext_hash: fnv(&ext) },
        );
    }
    write_registered(&registered)?;
    println!("analyse: {} copies registered; files under {}", pairs.len(), share.join("PIONEER/USBANLZ").display());
    Ok(())
}

/// The `.DAT` and `.EXT` for an analysis, section for section as rekordbox
/// writes its own (every header constant across 400 reference files), minus
/// the two we cannot author: `PSSI` (phrases) and `PVDI` (vocals). The path
/// rekordbox records is `?/<file name>` [OBS: all 300 sampled].
fn author(file_name: &str, analysis: &rbl_analysis::Analysis) -> (Vec<u8>, Vec<u8>) {
    let beats: Vec<rbl_anlz::Beat> = analysis
        .tempo
        .beats
        .iter()
        .map(|b| rbl_anlz::Beat { beat_number: b.beat_number, tempo_x100: b.tempo_x100, time_ms: b.time_ms })
        .collect();
    let ppth = format!("?/{file_name}");
    let wave = &analysis.waveform;

    let mut dat = rbl_anlz::AnlzBuilder::new();
    dat.path(&ppth)
        .vbr_table_zero()
        .beat_grid(&beats)
        .waveform_preview(b"PWAV", &wave.pack_preview())
        .waveform_preview(b"PWV2", &wave.pack_tiny())
        .cue_lists(false);

    let mut ext = rbl_anlz::AnlzBuilder::new();
    ext.path(&ppth)
        .waveform_scroll(b"PWV3", 1, &wave.pack_detail())
        .cue_lists(true)
        .extended_grid_empty()
        .waveform_scroll(b"PWV5", 2, &wave.pack_colour_detail())
        .waveform_scroll(b"PWV4", 6, &wave.pack_colour_preview());
    (dat.finish(), ext.finish())
}

// ---------------------------------------------------------------- report

fn grid_of(share: &Path, relative: &str) -> Vec<Beat> {
    if relative.is_empty() {
        return Vec::new();
    }
    rbl_anlz::Anlz::read(&rbl_anlz::resolve(share, relative))
        .ok()
        .and_then(|a| a.beat_grid())
        .map(|g| g.iter().map(|b| Beat { beat_number: b.beat_number, tempo_x100: b.tempo_x100, time_ms: b.time_ms }).collect())
        .unwrap_or_default()
}

fn report(filter: Option<&str>) -> Result<(), String> {
    let db = open_read_only()?;
    let share = db.location().share_root.clone();
    let pairs = pairs(db.connection(), filter)?;
    let (mut bpm_ok, mut down_ok, mut grid_ok, mut key_ok) = (0, 0, 0, 0);
    println!();
    for (copy, original) in &pairs {
        let ours = grid_of(&share, &copy.analysis_path);
        let rb = grid_of(&share, &original.analysis_path);
        let our_bpm = f64::from(copy.bpm_x100) / 100.0;
        let rb_bpm = f64::from(original.bpm_x100) / 100.0;
        let (down, _beat, in_bar) = offsets(&ours, &rb);
        let matched = grid_match(&ours, &rb);
        let bpm = (our_bpm - rb_bpm).abs() <= BPM_TOLERANCE;
        let downbeat = down.abs() <= MS_TOLERANCE;
        let grid = matched >= GRID_PASS;
        let key = copy.key == original.key && !copy.key.is_empty();
        bpm_ok += usize::from(bpm);
        down_ok += usize::from(downbeat);
        grid_ok += usize::from(grid);
        key_ok += usize::from(key);
        let mark = |ok: bool| if ok { "ok  " } else { "MISS" };
        println!("{}", original.title);
        println!("  bpm      {}  rb {:>7.2}  ours {:>7.2}", mark(bpm), rb_bpm, our_bpm);
        println!("  downbeat {}  {:+.0} ms, our beat 1 on rb beat {}", mark(downbeat), down, in_bar);
        println!("  grid     {}  {:.1} % of rb's {} beats matched by ours ({} beats)", mark(grid), matched * 100.0, rb.len(), ours.len());
        println!("  key      {}  rb {:<4} ours {:<4}", mark(key), original.key, copy.key);
        println!("  rb   runs: {}", runs_line(&rb));
        println!("  ours runs: {}", runs_line(&ours));
        if ours.is_empty() {
            println!("  (no grid registered for the copy; run `analyse`)");
        }
        println!();
    }
    let n = pairs.len();
    println!("summary: bpm {bpm_ok}/{n}  downbeat {down_ok}/{n}  grid {grid_ok}/{n}  key {key_ok}/{n}");
    Ok(())
}

// ---------------------------------------------------------------- check

fn check(filter: Option<&str>) -> Result<(), String> {
    let db = open_read_only()?;
    let share = db.location().share_root.clone();
    let registered = read_registered();
    if registered.is_empty() {
        return Err(format!("nothing recorded in {}; run `analyse` first", registered_file().display()));
    }
    let live: BTreeMap<String, Track> = members(db.connection(), RESULTS_PLAYLIST)?.into_iter().map(|t| (t.id.clone(), t)).collect();
    let mut touched = 0;
    for reg in registered.values().filter(|r| filter.is_none_or(|f| r.title.to_lowercase().contains(f))) {
        let mut notes = Vec::new();
        match live.get(&reg.id) {
            None => notes.push("row gone from the playlist or collection".to_owned()),
            Some(row) => {
                if row.analysis_path != reg.path {
                    notes.push(format!("AnalysisDataPath now {}", row.analysis_path));
                }
                if row.bpm_x100 != reg.bpm_x100 {
                    notes.push(format!("BPM now {:.2}", f64::from(row.bpm_x100) / 100.0));
                }
                if row.key != reg.key {
                    notes.push(format!("key now {:?}", row.key));
                }
                if row.analysed != Some(rbl_db::write::ANALYSED_FULL) {
                    notes.push(format!("Analysed now {:?}", row.analysed));
                }
                let dat = rbl_anlz::resolve(&share, &reg.path);
                match std::fs::read(&dat) {
                    Ok(bytes) if fnv(&bytes) == reg.dat_hash => {}
                    Ok(_) => notes.push("the .DAT was rewritten".to_owned()),
                    Err(_) => notes.push("the .DAT is gone".to_owned()),
                }
                match std::fs::read(rbl_anlz::sibling(&dat, "EXT")) {
                    Ok(bytes) if fnv(&bytes) == reg.ext_hash => {}
                    Ok(_) => notes.push("the .EXT was rewritten".to_owned()),
                    Err(_) => notes.push("the .EXT is gone".to_owned()),
                }
                for ext in ["2EX", "3EX"] {
                    if rbl_anlz::sibling(&dat, ext).exists() {
                        notes.push(format!("a .{ext} appeared"));
                    }
                }
                // What matters is whether the grid we wrote is still the one
                // rekordbox shows. On first load rekordbox rewrites the .EXT
                // waveforms and adds the .2EX/.3EX but keeps the .DAT [OBS].
                if notes.iter().any(|n| n.contains(".DAT")) {
                    notes.push("THE GRID WAS REPLACED".to_owned());
                } else if !notes.is_empty() {
                    notes.push("grid, BPM and key kept".to_owned());
                }
                if notes.is_empty() {
                    println!("  unchanged  {}  (updated_at {})", reg.title, row.updated_at);
                }
            }
        }
        if !notes.is_empty() {
            touched += 1;
            println!("  TOUCHED    {}: {}", reg.title, notes.join("; "));
        }
    }
    println!("check: {touched} of {} registered copies touched since `analyse`", registered.len());
    Ok(())
}

// ---------------------------------------------------------------- plumbing

fn read_registered() -> BTreeMap<String, Registered> {
    let Ok(text) = std::fs::read_to_string(registered_file()) else {
        return BTreeMap::new();
    };
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 7 {
                return None;
            }
            Some((
                f[0].to_owned(),
                Registered {
                    id: f[0].to_owned(),
                    title: f[1].to_owned(),
                    path: f[2].to_owned(),
                    bpm_x100: f[3].parse().ok()?,
                    key: f[4].to_owned(),
                    dat_hash: f[5].parse().ok()?,
                    ext_hash: f[6].parse().ok()?,
                },
            ))
        })
        .collect()
}

fn write_registered(all: &BTreeMap<String, Registered>) -> Result<(), String> {
    let path = registered_file();
    let mut file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    for r in all.values() {
        writeln!(file, "{}\t{}\t{}\t{}\t{}\t{}\t{}", r.id, r.title.replace('\t', " "), r.path, r.bpm_x100, r.key, r.dat_hash, r.ext_hash)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

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

/// FNV-1a, enough to notice a rewritten file.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_owned() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
