//! Bind the synthetic audio/ANLZ prepared by tools/debug/sync-tempo-20261013
//! to an isolated encrypted library. Refuses to overwrite an existing database.
use rbl_db::fixture::{self, Shape, FIXTURE_PASSPHRASE};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args_os().nth(1).ok_or("usage: sync_fixture <prepared-directory>")?;
    let dir = std::fs::canonicalize(dir)?;
    if dir.join("master.db").exists() { return Err("fixture database already exists".into()); }
    let location = fixture::build(&dir, Shape { tracks: 3, playlists: 1,
        tracks_per_playlist: 3, history_sessions: 0, ..Default::default() })?;
    for index in 0..3 {
        let audio = dir.join(format!("audio/Sync {index}.wav"));
        let analysis = format!("/PIONEER/USBANLZ/sync-{index}/ANLZ0000.DAT");
        if !audio.is_file() || !location.share_root.join(analysis.trim_start_matches('/')).is_file() {
            return Err("prepare the fixture audio and analysis first".into());
        }
        fixture::point_at_audio(&location, index, audio.to_str().ok_or("non-UTF-8 fixture path")?, 90)?;
        fixture::set_analysis_path(&location, index, &analysis)?;
        fixture::set_tempo(&location, index, if index < 2 { 10000 } else { 12000 })?;
    }
    fixture::write_options_json(&dir.join("options.json"),
        location.master_db.to_str().ok_or("non-UTF-8 fixture path")?, FIXTURE_PASSPHRASE)?;
    Ok(())
}
