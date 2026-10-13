//! Regenerate RBX waveforms and import embedded covers in a named playlist.
//! Preserves grids, cues, BPM, keys and all other analysis sections. Backs up
//! every changed analysis file; Writer backs up the database before writing.
//! cargo run --release -p rbl-analysis --example `repair_waveforms` -- RBX-BPM-MULTIBPM-RESULTS
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use rbl_anlz::Anlz;
use rbl_db::write::Writer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let playlist = std::env::args().nth(1).expect("playlist name required");
    let overview_only = std::env::args().any(|arg| arg == "--overview-only");
    let location = rbl_db::detect()?;
    let share = location.share_root.clone();
    let backup = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verification/preview-repair")
        .join(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string());
    let mut writer = Writer::open(location, backup.join("database"))?;
    let rows: Vec<(String, String, String)> = {
        let mut stmt = writer.library().connection().prepare(
            "SELECT c.ID, c.Title, c.AnalysisDataPath FROM djmdContent c
             JOIN djmdSongPlaylist sp ON sp.ContentID = c.ID
             JOIN djmdPlaylist p ON p.ID = sp.PlaylistID
             WHERE p.Name = ?1 AND p.rb_local_deleted = 0
             AND sp.rb_local_deleted = 0 AND c.rb_local_deleted = 0 ORDER BY sp.TrackNo")?;
        let rows = stmt.query_map([playlist], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        rows
    };
    assert!(!rows.is_empty(), "playlist is empty or missing");
    println!("Backup: {}", backup.display());
    for (id, title, relative) in rows {
        let artwork = if overview_only { false } else { writer.import_artwork(&id)? };
        let bpm: u32 = writer.library().connection().query_row(
            "SELECT COALESCE(BPM, 0) FROM djmdContent WHERE ID = ?1", [&id], |r| r.get(0))?;
        let dat = rbl_anlz::resolve(&share, &relative);
        let audio_path: String = writer.library().connection().query_row(
            "SELECT FolderPath FROM djmdContent WHERE ID = ?1", [&id], |r| r.get(0))?;
        let audio = rbl_audio::decode_mono(std::path::Path::new(&audio_path), Some(1800.0))?;
        let waveform = rbl_analysis::waveform::compute(&audio.samples, audio.sample_rate);
        let columns: Vec<_> = waveform.columns.iter().map(|c| rbl_anlz::BandColumn {
            low: c.low, mid: c.mid, high: c.high, peak: c.peak,
        }).collect();
        let generated = rbl_anlz::author_with_overview(&audio_path, &[], &columns, waveform.overview.as_slice().try_into().ok(), rbl_anlz::Existing::default());
        for (extension, bytes) in [("DAT", generated.dat), ("EXT", generated.ext), ("2EX", generated.two_ex)] {
            let path = dat.with_extension(extension);
            let mut file = Anlz::read(&path)?;
            let replacement = rbl_anlz::parse(&bytes)?;
            let mut changed = false;
            for section in &mut file.sections {
                // Replace waveforms only: keep all grids, cue lists, phrases,
                // vocals and unknown records byte-for-byte.
                if section.waveform().is_some() && (!overview_only || section.tag.to_string() == "PWV6") {
                    if let Some(new) = replacement.sections.iter().find(|s| s.tag == section.tag) {
                        if section.header != new.header || section.payload != new.payload {
                            *section = new.clone();
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                if rbl_db::is_rekordbox_running() { return Err("quit rekordbox before repairing".into()); }
                let original = backup.join(&id).join(path.file_name().unwrap());
                std::fs::create_dir_all(original.parent().unwrap())?;
                std::fs::copy(&path, original)?;
                let staged = path.with_extension(format!("{extension}.tmp"));
                std::fs::write(&staged, file.to_bytes())?;
                std::fs::rename(staged, path)?;
            }
        }
        if !overview_only { writer.set_analysis(&id, &rbl_db::write::AnalysisWrite {
            bpm_x100: bpm, key: None, analysis_path: &relative, length_sec: None,
        })?; }
        println!("{title}: preview checked, artwork imported={artwork}");
    }
    Ok(())
}
