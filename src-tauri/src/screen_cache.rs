//! The media for the last browser screen, persisted only on a graceful exit.
//!
//! Row text is already restored synchronously from the frontend's session
//! snapshot. Keeping the matching artwork and overview waveform bytes here
//! lets those seeded rows paint while the real library is still opening.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use tauri::Manager;

use crate::state::AppState;

const DIRECTORY: &str = "last-screen-v1";
const MAX_TRACKS: usize = 200;

#[derive(Default)]
struct Selection {
    ids: Vec<String>,
    waveform_kind: String,
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();
static SELECTION: OnceLock<Mutex<Selection>> = OnceLock::new();

pub fn initialize(app: &tauri::AppHandle) {
    if let Ok(cache) = app.path().app_cache_dir() {
        let _ = ROOT.set(cache.join(DIRECTORY));
    }
}

#[tauri::command]
pub fn remember_screen_assets(ids: Vec<String>, waveform_kind: String) {
    let ids = ids
        .into_iter()
        .filter(|id| id.parse::<u64>().is_ok())
        .take(MAX_TRACKS)
        .collect();
    *SELECTION.get_or_init(Default::default).lock() = Selection { ids, waveform_kind };
}

pub fn cached_waveform(track_id: &str, kind: &str) -> Option<Vec<u8>> {
    std::fs::read(root()?.join("waveforms").join(safe_name(track_id, kind)?)).ok()
}

pub fn cached_artwork(track_id: &str) -> Option<Vec<u8>> {
    std::fs::read(root()?.join("artwork").join(safe_id(track_id)?)).ok()
}

pub fn save(app: &tauri::AppHandle) {
    let Some(root) = root() else { return };
    let selection = SELECTION.get_or_init(Default::default).lock();
    if selection.ids.is_empty() || selection.waveform_kind.is_empty() {
        return;
    }
    let state = app.state::<Arc<AppState>>();
    let Ok(library) = state.library() else { return };
    let share = state.share_root();
    let next = root.with_extension("next");
    let _ = std::fs::remove_dir_all(&next);
    if std::fs::create_dir_all(next.join("artwork")).is_err()
        || std::fs::create_dir_all(next.join("waveforms")).is_err()
    {
        return;
    }

    for id in &selection.ids {
        if let Some(relative) = library.artwork_path_of(id).filter(|path| !path.is_empty()) {
            if let Some(source) = crate::protocol::resolve_under(&share, relative) {
                let _ = copy_bounded(&source, &next.join("artwork").join(id), crate::protocol::MAX_BYTES);
            }
        }
        if let Ok(bytes) = crate::commands::waveform_bytes(
            &library,
            &share,
            id,
            &selection.waveform_kind,
            None,
            None,
        ) {
            if !bytes.is_empty() {
                let _ = std::fs::write(next.join("waveforms").join(format!("{}-{}", id, selection.waveform_kind)), bytes);
            }
        }
    }

    let old = root.with_extension("old");
    let _ = std::fs::remove_dir_all(&old);
    let had_old = root.exists() && std::fs::rename(&root, &old).is_ok();
    if let Err(error) = std::fs::rename(&next, &root) {
        if had_old {
            let _ = std::fs::rename(&old, &root);
        }
        tracing::warn!(%error, "could not publish the last-screen media cache");
    } else if had_old {
        let _ = std::fs::remove_dir_all(&old);
    }
}

fn root() -> Option<PathBuf> {
    ROOT.get().cloned()
}

fn safe_id(track_id: &str) -> Option<&str> {
    track_id.parse::<u64>().ok().map(|_| track_id)
}

fn safe_name(track_id: &str, kind: &str) -> Option<String> {
    safe_id(track_id)?;
    matches!(kind, "bands" | "colour" | "color" | "mono").then(|| format!("{track_id}-{kind}"))
}

fn copy_bounded(source: &Path, destination: &Path, max: u64) -> std::io::Result<()> {
    if std::fs::metadata(source)?.len() > max {
        return Ok(());
    }
    std::fs::copy(source, destination).map(|_| ())
}
