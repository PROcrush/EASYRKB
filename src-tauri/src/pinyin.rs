//! App-local pronunciation corrections. The rekordbox database and audio tags
//! are never opened for writing by these commands.
use std::{collections::HashMap, hash::{Hash, Hasher}, path::{Path, PathBuf}, sync::Arc};
use rbl_index::pinyin::{Reading, ReadingOverride, TrackOverride};
use serde::Serialize;
use tauri::{Emitter, State};
use crate::{AppError, AppResult, ErrorKind, state::AppState};

pub(crate) fn path(directory: &Path, database: &Path) -> PathBuf {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    database.hash(&mut hash);
    directory.join(format!("pinyin-{:016x}.json", hash.finish()))
}
pub(crate) fn read(path: &Path) -> AppResult<HashMap<u64, TrackOverride>> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Ok(meta) if meta.len() <= 8 * 1024 * 1024 => {},
        _ => return Err(AppError::new(ErrorKind::Malformed, "The local pinyin corrections could not be read.")),
    }
    let data = std::fs::read(path).map_err(|e| AppError::internal(e.to_string()))?;
    serde_json::from_slice(&data).map_err(|_| AppError::new(ErrorKind::Malformed, "The local pinyin corrections could not be read."))
}
pub(crate) fn save(path: &Path, corrections: &HashMap<u64, TrackOverride>) -> AppResult<()> {
    let directory = path.parent().ok_or_else(|| AppError::internal("Missing pinyin directory"))?;
    std::fs::create_dir_all(directory).map_err(|e| AppError::internal(e.to_string()))?;
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| AppError::internal(e.to_string()))?;
    serde_json::to_writer(&mut file, corrections).map_err(|e| AppError::internal(e.to_string()))?;
    file.as_file().sync_all().map_err(|e| AppError::internal(e.to_string()))?;
    file.persist(path).map_err(|e| AppError::internal(e.to_string()))?;
    Ok(())
}
fn correction(original: String, reading: &str) -> AppResult<Option<ReadingOverride>> {
    let reading = reading.trim().to_lowercase().replace('ü', "v");
    if reading.is_empty() { return Ok(None); }
    if reading.len() > 2048 || !reading.chars().all(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace()) {
        return Err(AppError::new(ErrorKind::Malformed, "Enter pinyin syllables separated by spaces, using v for ü."));
    }
    Ok(Some(ReadingOverride { original, reading: reading.split_whitespace().collect::<Vec<_>>().join(" ") }))
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress { job_id: String, done: usize, total: usize }
#[derive(Serialize)]
pub struct Generated { processed: usize, chinese: usize, skipped: usize }

#[tauri::command]
pub async fn pinyin_reading(state: State<'_, Arc<AppState>>, id: String) -> AppResult<Reading> {
    let state = Arc::clone(state.inner());
    crate::commands::blocking("pinyin_reading", move || state.library()?.pinyin_reading(&id).ok_or_else(|| AppError::new(ErrorKind::NotFound, "Choose a track from the collection."))).await
}
#[tauri::command]
#[allow(clippy::too_many_arguments, reason = "IPC includes the original text to reject stale corrections")]
pub async fn pinyin_save(app: tauri::AppHandle, state: State<'_, Arc<AppState>>, id: String, original_title: String, original_artist: String, title: String, artist: String) -> AppResult<Reading> {
    let state = Arc::clone(state.inner());
    let change = TrackOverride { title: correction(original_title.clone(), &title)?, artist: correction(original_artist.clone(), &artist)? };
    let result = crate::commands::blocking("pinyin_save", move || state.save_pinyin(&id, &original_title, &original_artist, change)).await?;
    let _ = app.emit("library:changed", result.0);
    Ok(result.1)
}
#[tauri::command]
pub async fn pinyin_generate(app: tauri::AppHandle, state: State<'_, Arc<AppState>>, tracks: Option<Vec<String>>, job_id: String) -> AppResult<Generated> {
    let state = Arc::clone(state.inner());
    let result = crate::commands::blocking("pinyin_generate", move || {
        let (generation, processed, chinese, skipped) = state.generate_pinyin(tracks, |done, total| { let _ = app.emit("pinyin:progress", Progress { job_id: job_id.clone(), done, total }); })?;
        let _ = app.emit("library:changed", generation);
        Ok(Generated { processed, chinese, skipped })
    }).await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_library_keeps_local_readings_across_reload_and_regeneration() {
        use rbl_index::testing::{library_from, TestTrack};
        let temp = tempfile::tempdir().unwrap();
        let location = rbl_db::fixture::build(temp.path(), rbl_db::fixture::Shape::default()).unwrap();
        let before = std::fs::read(&location.master_db).unwrap();
        let library = || library_from(&[TestTrack { id: 1, title: "重庆森林", artist: "王菲", ..TestTrack::default() }]);
        let state = AppState::with_backups(temp.path().join("settings"));
        state.set_library(library(), true, None, 0, location.clone());
        let change = TrackOverride { title: correction("重庆森林".into(), "zhong qing sen lin").unwrap(), artist: None };
        state.save_pinyin("1", "重庆森林", "王菲", change).unwrap();
        state.generate_pinyin(None, |_, _| {}).unwrap();
        state.set_library(library(), true, None, 0, location.clone());
        assert_eq!(state.library().unwrap().pinyin_reading("1").unwrap().title_initials, "zqsl");
        assert!(state.save_pinyin("1", "旧名称", "王菲", TrackOverride::default()).is_err());
        assert_eq!(std::fs::read(&location.master_db).unwrap(), before);
    }
    #[test]
    fn corrections_persist_separately_for_each_library_and_reject_bad_input() {
        let temp = tempfile::tempdir().unwrap();
        let first = path(temp.path(), Path::new("/library-a/master.db"));
        let second = path(temp.path(), Path::new("/library-b/master.db"));
        let change = TrackOverride { title: correction("重庆".into(), "chong qing").unwrap(), artist: None };
        save(&first, &HashMap::from([(42, change)])).unwrap();
        assert_eq!(read(&first).unwrap()[&42].title.as_ref().unwrap().reading, "chong qing");
        assert!(read(&second).unwrap().is_empty());
        assert!(correction("女".into(), "nü").unwrap().is_some());
        assert!(correction("重庆".into(), "中文").is_err());
        std::fs::write(&first, b"broken JSON").unwrap();
        assert!(read(&first).is_err());
    }
}
