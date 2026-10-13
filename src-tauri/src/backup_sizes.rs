//! The Rekordbox Data estimate: logical bytes in the next library backup,
//! measured by [`rbl_backup::sizes`] and kept for a week.
use crate::{
    error::{AppError, AppResult},
    state::AppState,
};
pub use rbl_backup::sizes::BackupSizes;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const WEEK_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Shared in memory across windows and persisted across application restarts.
#[derive(Default)]
pub struct SizeCache {
    value: Option<AppResult<BackupSizes>>,
    library: Option<(PathBuf, PathBuf)>,
}
impl SizeCache {
    fn get(
        &mut self,
        refresh: bool,
        scan: impl FnOnce() -> AppResult<BackupSizes>,
    ) -> AppResult<BackupSizes> {
        if !refresh {
            if let Some(value) = &self.value {
                return value.clone();
            }
        }
        let result = scan();
        // Keep a previous successful reading if a manual refresh fails.
        // Cache an initial failure too; retrying must be an explicit request.
        if result.is_ok() || self.value.is_none() {
            self.value = Some(result.clone());
        }
        result
    }
}

#[derive(Deserialize, Serialize)]
struct SavedSizes {
    version: u32,
    database: PathBuf,
    analysis: PathBuf,
    sizes: BackupSizes,
}

impl SizeCache {
    fn persisted(
        &mut self,
        path: &Path,
        database: &Path,
        analysis: &Path,
        refresh: bool,
        scan: impl FnOnce() -> AppResult<BackupSizes>,
    ) -> AppResult<BackupSizes> {
        let identity = (database.to_path_buf(), analysis.to_path_buf());
        if self.library.as_ref() != Some(&identity) {
            self.value = None;
            self.library = Some(identity);
        }
        if self.value.is_none() {
            if let Some(saved) = fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<SavedSizes>(&bytes).ok())
            {
                if saved.version == 2 && saved.database == database && saved.analysis == analysis {
                    self.value = Some(Ok(saved.sizes));
                }
            }
        }
        let now = rbl_core::time::unix_millis();
        let stale = self.value.as_ref().is_some_and(|value| {
            value.as_ref().is_ok_and(|sizes| now.saturating_sub(sizes.updated_at) >= WEEK_MS)
        });
        self.get(refresh || stale, || {
            let sizes = scan()?;
            let saved = SavedSizes {
                version: 2,
                database: database.into(),
                analysis: analysis.into(),
                sizes: sizes.clone(),
            };
            let persist = || -> Result<(), Box<dyn std::error::Error>> {
                if let Some(parent) = path.parent() {
                    crate::durable::create_dir_all(parent)?;
                }
                crate::durable::write(path, &serde_json::to_vec(&saved)?)?;
                Ok(())
            };
            if let Err(error) = persist() {
                tracing::warn!(%error, "Could not persist backup size estimate");
            }
            Ok(sizes)
        })
    }
}

pub fn cached(state: &AppState, refresh: bool) -> AppResult<BackupSizes> {
    let location = state.location()?;
    // Loading the saved reading requires one small JSON read, no library scan.
    // Reuse it for a week; manual Refresh can request a new reading sooner.
    state.backup_sizes.lock().persisted(
        &state.backup_dir().join(".size-cache.json"),
        &location.master_db,
        &location.share_root.join("PIONEER/USBANLZ"),
        refresh,
        || measure(state),
    )
}

fn measure(state: &AppState) -> AppResult<BackupSizes> {
    let location = state.location()?;
    let track_count = state.read_db(rbl_db::Library::live_track_count).map_err(|e| AppError::internal(e.to_string()))?;
    rbl_backup::sizes::measure_paths(
        &location.master_db,
        &rbl_backup::analysis_dir(&location),
        &rbl_backup::artwork_dir(&location),
    )
    .map(|measured| {
        let mut sizes = measured.sizes;
        sizes.track_count = track_count;
        sizes.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        sizes
    })
    .map_err(|e| AppError::internal(format!("Could not measure backup contents: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, reason = "the panic in a scan closure proves the cache never called it")]
mod tests {
    use super::*;
    #[test]
    fn week_old_readings_refresh_automatically_and_persist_the_new_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sizes.json");
        let db = dir.path().join("master.db");
        let analysis = dir.path().join("analysis");
        let now = rbl_core::time::unix_millis();
        SizeCache::default().persisted(&path, &db, &analysis, false, || {
            Ok(BackupSizes { updated_at: now - WEEK_MS - 1, database: 10, ..Default::default() })
        }).unwrap();
        let mut restarted = SizeCache::default();
        assert!(restarted.persisted(&path, &db, &analysis, false, || Err(AppError::internal("offline"))).is_err());
        let saved: SavedSizes = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.sizes.database, 10);
        let refreshed = restarted.persisted(&path, &db, &analysis, false, || {
            Ok(BackupSizes { updated_at: now, database: 20, ..Default::default() })
        }).unwrap();
        assert_eq!(refreshed.database, 20);
        let saved = SizeCache::default().persisted(&path, &db, &analysis, false, || {
            panic!("a fresh weekly estimate must be reused")
        }).unwrap();
        assert_eq!(saved.database, 20);
        assert_eq!(saved.updated_at, now);
    }

    #[test]
    fn saved_reading_survives_restart_and_only_refresh_rescans() {
        let now = rbl_core::time::unix_millis();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sizes.json");
        let db = dir.path().join("master.db");
        let analysis = dir.path().join("analysis");
        let mut first = SizeCache::default();
        first
            .persisted(&path, &db, &analysis, false, || {
                Ok(BackupSizes {
                    updated_at: now,
                    database: 100,
                    waveforms: 200,
                    ..Default::default()
                })
            })
            .unwrap();
        let mut restarted = SizeCache::default();
        let saved = restarted
            .persisted(&path, &db, &analysis, false, || {
                panic!("must not scan after restart")
            })
            .unwrap();
        assert_eq!(
            (saved.updated_at, saved.database, saved.waveforms),
            (now, 100, 200)
        );
        restarted
            .persisted(&path, &db, &analysis, true, || {
                Ok(BackupSizes {
                    updated_at: now + 1,
                    database: 300,
                    ..Default::default()
                })
            })
            .unwrap();
        let saved = SizeCache::default()
            .persisted(&path, &db, &analysis, false, || {
                panic!("refresh should persist")
            })
            .unwrap();
        assert_eq!((saved.updated_at, saved.database), (now + 1, 300));
    }

    #[test]
    fn failed_refresh_keeps_the_persisted_success() {
        let now = rbl_core::time::unix_millis();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sizes.json");
        let db = dir.path().join("master.db");
        let analysis = dir.path().join("analysis");
        let mut cache = SizeCache::default();
        cache
            .persisted(&path, &db, &analysis, false, || {
                Ok(BackupSizes {
                    updated_at: now,
                    ..Default::default()
                })
            })
            .unwrap();
        assert!(cache
            .persisted(&path, &db, &analysis, true, || Err(AppError::internal(
                "offline"
            )))
            .is_err());
        assert_eq!(
            SizeCache::default()
                .persisted(&path, &db, &analysis, false, || panic!(
                    "keep saved reading"
                ))
                .unwrap()
                .updated_at,
            now
        );
    }

    #[test]
    fn corrupt_cache_and_different_library_require_a_new_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sizes.json");
        let db = dir.path().join("master.db");
        let analysis = dir.path().join("analysis");
        fs::write(&path, b"broken json").unwrap();
        let mut cache = SizeCache::default();
        cache
            .persisted(&path, &db, &analysis, false, || {
                Ok(BackupSizes {
                    updated_at: 1,
                    ..Default::default()
                })
            })
            .unwrap();
        let other = dir.path().join("other.db");
        assert_eq!(
            cache
                .persisted(&path, &other, &analysis, false, || Ok(BackupSizes {
                    updated_at: 2,
                    ..Default::default()
                }))
                .unwrap()
                .updated_at,
            2
        );
    }

    #[test]
    fn scans_once_per_session_until_explicit_refresh() {
        let scans = std::cell::Cell::new(0);
        let scan = || {
            scans.set(scans.get() + 1);
            Ok(BackupSizes {
                updated_at: scans.get(),
                ..Default::default()
            })
        };
        let mut cache = SizeCache::default();
        assert_eq!(scans.get(), 0);
        assert_eq!(cache.get(false, scan).unwrap().updated_at, 1);
        assert_eq!(cache.get(false, scan).unwrap().updated_at, 1);
        assert_eq!(scans.get(), 1);
        assert_eq!(cache.get(true, scan).unwrap().updated_at, 2);
        assert_eq!(cache.get(false, scan).unwrap().updated_at, 2);
        let mut next_session = SizeCache::default();
        assert_eq!(next_session.get(false, scan).unwrap().updated_at, 3);
    }

    #[test]
    fn failed_scans_require_refresh_and_preserve_previous_success() {
        let mut cache = SizeCache::default();
        assert!(cache
            .get(false, || Err(AppError::internal("unavailable")))
            .is_err());
        assert!(cache.get(false, || Ok(BackupSizes::default())).is_err());
        assert!(cache
            .get(true, || Ok(BackupSizes {
                updated_at: 42,
                ..Default::default()
            }))
            .is_ok());
        assert!(cache
            .get(true, || Err(AppError::internal("unavailable")))
            .is_err());
        assert_eq!(
            cache
                .get(false, || Ok(BackupSizes::default()))
                .unwrap()
                .updated_at,
            42
        );
    }
}
