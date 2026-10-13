//! USB recovery commands keep trusted snapshots in the backend; IPC carries IDs.
use crate::error::{AppError, AppResult, ErrorKind};
use rbl_export::recovery::{Outcome, Preview, RecoveryError, Report, Session};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager, State};

#[derive(Default)]
pub struct RecoveryStore {
    state: Mutex<RecoveryState>,
}
#[derive(Default)]
struct RecoveryState {
    sessions: HashMap<String, Arc<Session>>,
    jobs: HashMap<String, Arc<AtomicBool>>,
    clearing: bool,
}
struct Job {
    id: String,
    cancel: Arc<AtomicBool>,
    store: Arc<RecoveryStore>,
    app: Option<tauri::AppHandle>,
}
impl Job {
    fn notify(mut self, app: &tauri::AppHandle) -> Self {
        self.app = Some(app.clone());
        let _ = app.emit("usb-recovery:cache-changed", false);
        self
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Ok(mut state) = self.store.state.lock() {
            state.jobs.remove(&self.id);
        }
        if let Some(app) = &self.app {
            let _ = app.emit("usb-recovery:cache-changed", false);
        }
    }
}
struct CacheClear {
    store: Arc<RecoveryStore>,
    sessions: Option<HashMap<String, Arc<Session>>>,
}
impl Drop for CacheClear {
    fn drop(&mut self) {
        if let Ok(mut state) = self.store.state.lock() {
            state.clearing = false;
        }
    }
}
impl RecoveryStore {
    fn start_job(state: &mut RecoveryState, id: &str) -> AppResult<Arc<AtomicBool>> {
        if state.clearing || state.jobs.len() >= 3 || state.jobs.contains_key(id) {
            return Err(AppError::new(ErrorKind::Malformed, "A recovery operation is already running."));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        state.jobs.insert(id.into(), Arc::clone(&cancel));
        Ok(cancel)
    }
    fn job(self: &Arc<Self>, id: &str) -> AppResult<Job> {
        let mut state = self.state
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        let cancel = Self::start_job(&mut state, id)?;
        Ok(Job {
            id: id.into(),
            cancel,
            store: Arc::clone(self),
            app: None,
        })
    }
    // Session lookup and job registration must be atomic with cache clearing.
    fn session_job(self: &Arc<Self>, report: &str, job: &str) -> AppResult<(Arc<Session>, Job)> {
        let mut state = self.state
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        let session = state.sessions.get(report)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    ErrorKind::NotFound,
                    "The recovery snapshot expired. Diagnose the drive again.",
                )
            })?;
        let cancel = Self::start_job(&mut state, job)?;
        Ok((session, Job { id: job.into(), cancel, store: Arc::clone(self), app: None }))
    }
    fn begin_clear(self: &Arc<Self>) -> AppResult<CacheClear> {
        let mut state = self.state.lock().map_err(|e| AppError::internal(e.to_string()))?;
        if state.clearing || !state.jobs.is_empty() {
            return Err(AppError::new(ErrorKind::Malformed, "Wait for diagnosis or recovery to finish before clearing the cache."));
        }
        state.clearing = true;
        let sessions = std::mem::take(&mut state.sessions);
        Ok(CacheClear { store: Arc::clone(self), sessions: Some(sessions) })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    path: String,
    bytes: u64,
    busy: bool,
    sessions: usize,
}
fn cache_info(path: &Path, store: &RecoveryStore) -> AppResult<CacheInfo> {
    let bytes = crate::usb_recovery_cache::usage(path).map_err(|e| AppError::internal(e.to_string()))?;
    let state = store.state.lock().map_err(|e| AppError::internal(e.to_string()))?;
    Ok(CacheInfo { path: path.to_string_lossy().into(), bytes, busy: state.clearing || !state.jobs.is_empty(), sessions: state.sessions.len() })
}

#[tauri::command]
pub async fn usb_recovery_cache_info(app: tauri::AppHandle, store: State<'_, Arc<RecoveryStore>>) -> AppResult<CacheInfo> {
    let path = cache(&app)?;
    let store = Arc::clone(store.inner());
    crate::commands::blocking("usb_recovery_cache_info", move || cache_info(&path, &store)).await
}

#[tauri::command]
pub async fn usb_recovery_clear_cache(app: tauri::AppHandle, store: State<'_, Arc<RecoveryStore>>) -> AppResult<CacheInfo> {
    let path = cache(&app)?;
    let store = Arc::clone(store.inner());
    let mut clearing = store.begin_clear()?;
    let _ = app.emit("usb-recovery:cache-changed", false);
    crate::commands::blocking("usb_recovery_clear_cache", move || {
        // Dropping inactive sessions removes their TempDirs. Repair backups live
        // in app_data_dir, outside this cache, and are never enumerated here.
        drop(clearing.sessions.take());
        let result = crate::usb_recovery_cache::clear(&path).map_err(|e| AppError::internal(e.to_string()));
        drop(clearing);
        // Even partial deletion invalidates the old previews in every window.
        let _ = app.emit("usb-recovery:cache-changed", true);
        result?;
        cache_info(&path, &store)
    }).await
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    report_id: String,
    report: Report,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    outcome: Outcome,
    report_path: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    job_id: String,
    stage: String,
    done: usize,
    total: usize,
}
#[allow(clippy::needless_pass_by_value, reason = "Result::map_err supplies an owned recovery error")]
fn failure(e: RecoveryError) -> AppError {
    match e {
        RecoveryError::Healthy => {
            return AppError::new(
                ErrorKind::Malformed,
                "The device databases already match. Missing files require a backup or relocation.",
            )
        }
        RecoveryError::PendingPublication => {
            return AppError::new(
                ErrorKind::Malformed,
                "An interrupted publication must be resolved before repairing this drive.",
            )
        }
        _ => {}
    }
    let kind = if matches!(
        e,
        RecoveryError::Cancelled | RecoveryError::Export(rbl_export::ExportError::Cancelled)
    ) {
        ErrorKind::Cancelled
    } else {
        ErrorKind::Malformed
    };
    AppError::new(kind, e.to_string())
}
fn cache(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_cache_dir()
        .map(|p| p.join("usb-recovery"))
        .map_err(|e| AppError::internal(e.to_string()))
}
fn backups(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("usb-recovery/backups"))
        .map_err(|e| AppError::internal(e.to_string()))
}
fn progress(app: &tauri::AppHandle, id: &str, stage: &str, done: usize, total: usize) {
    let _ = app.emit(
        "usb-recovery:progress",
        Progress {
            job_id: id.into(),
            stage: stage.into(),
            done,
            total,
        },
    );
}
fn report_path(cache: &Path, id: &str) -> AppResult<PathBuf> {
    uuid::Uuid::parse_str(id)
        .map_err(|_| AppError::new(ErrorKind::Malformed, "Invalid recovery report ID."))?;
    Ok(cache.join(format!("EASYRKB-{id}.json")))
}
fn save_report(path: &Path, report: &Report, result: &impl Serialize) -> AppResult<()> {
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| AppError::internal("Missing recovery cache"))?,
    )
    .map_err(|e| AppError::internal(e.to_string()))?;
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
        .map_err(|e| AppError::internal(e.to_string()))?;
    serde_json::to_writer_pretty(&mut file, &serde_json::json!({ "app": "EASYRKB", "source": report, "result": result, "validation": "Structural checks only; player hardware has not been tested.", "xmlLimitations": "XML cannot preserve unknown ANLZ sections or all player-specific cue and loop attributes." })).map_err(|e| AppError::internal(e.to_string()))?;
    file.as_file()
        .sync_all()
        .map_err(|e| AppError::internal(e.to_string()))?;
    file.persist(path)
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn usb_recovery_diagnose(
    app: tauri::AppHandle,
    store: State<'_, Arc<RecoveryStore>>,
    path: String,
    job_id: String,
    cache_limit_gib: Option<u64>,
) -> AppResult<Diagnosis> {
    let cache_limit_gib = cache_limit_gib.unwrap_or(8);
    if !matches!(cache_limit_gib, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
        return Err(AppError::new(ErrorKind::Malformed, "Choose a diagnosis cache limit from 1 to 64 GiB."));
    }
    let store = Arc::clone(store.inner());
    let job = store.job(&job_id)?.notify(&app);
    let cache = cache(&app)?;
    crate::commands::blocking("usb_recovery_diagnose", move || {
        // Bound retained snapshots, including concurrent diagnoses at insertion time.
        if store
            .state
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?
            .sessions.len()
            >= 3
        {
            return Err(AppError::new(
                ErrorKind::Malformed,
                "Close another recovery window before diagnosing a new drive.",
            ));
        }
        let session = Session::diagnose_with_limit(
            Path::new(&path),
            &cache,
            cache_limit_gib * 1024 * 1024 * 1024,
            &|| job.cancel.load(Ordering::Relaxed),
            &mut |stage, done, total| progress(&app, &job.id, stage, done, total),
        )
        .map_err(failure)?;
        let id = uuid::Uuid::new_v4().to_string();
        save_report(
            &report_path(&cache, &id)?,
            &session.report,
            &serde_json::json!({"status":"diagnosed"}),
        )?;
        let diagnosis = Diagnosis {
            report_id: id.clone(),
            report: session.report.clone(),
        };
        let mut state = store
            .state
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        if state.sessions.len() >= 3 {
            return Err(AppError::new(
                ErrorKind::Malformed,
                "Close another recovery window before diagnosing a new drive.",
            ));
        }
        state.sessions.insert(id, Arc::new(session));
        Ok(diagnosis)
    })
    .await
}
#[tauri::command]
pub async fn usb_recovery_preview(
    app: tauri::AppHandle,
    store: State<'_, Arc<RecoveryStore>>,
    report_id: String,
    source_id: String,
    offset: usize,
) -> AppResult<Preview> {
    let (session, job) = Arc::clone(store.inner()).session_job(&report_id, &uuid::Uuid::new_v4().to_string())?;
    let job = job.notify(&app);
    crate::commands::blocking("usb_recovery_preview", move || {
        let _job = job;
        session.preview(&source_id, offset, 100).map_err(failure)
    })
    .await
}
#[tauri::command]
pub async fn usb_recovery_execute(
    app: tauri::AppHandle,
    store: State<'_, Arc<RecoveryStore>>,
    report_id: String,
    source_id: String,
    job_id: String,
    kind: String,
    destination: String,
) -> AppResult<Execution> {
    let (session, job) = Arc::clone(store.inner()).session_job(&report_id, &job_id)?;
    let job = job.notify(&app);
    let report_path = report_path(&cache(&app)?, &report_id)?;
    crate::commands::blocking("usb_recovery_execute", move || {
        let run = || job.cancel.load(Ordering::Relaxed);
        let mut emit = |stage: &str, done, total| progress(&app, &job.id, stage, done, total);
        let result = match kind.as_str() {
            "repair" => session.repair_current(&source_id, &backups(&app)?, &run, &mut emit),
            "usb" => {
                if let Some(device) = rbl_devices::list().iter().find(|d| d.mount_point == Path::new(&destination)) {
                    let required = session.report.sources.iter().find(|s| s.id == source_id).map_or(u64::MAX, |s| s.required_bytes);
                    if device.total_bytes > 0 && device.free_bytes < required { return Err(AppError::new(ErrorKind::Malformed, "The destination does not have enough free space for recovery.")); }
                }
                session.restore_to_empty(&source_id, Path::new(&destination), &run, &mut emit)
            }
            "xml" => session.write_xml(&source_id, Path::new(&destination), &run, &mut emit),
            _ => return Err(AppError::new(ErrorKind::Malformed, "Choose a recovery output format.")),
        };
        match result {
            Ok(outcome) => {
                // Report errors after a successful publish must never mislabel the USB as unmodified.
                if let Err(e) = save_report(&report_path, &session.report, &outcome) { tracing::error!(error = %e, "recovery published but report could not be updated"); return Err(AppError::new(ErrorKind::Internal, "Recovery output was saved, but the local report could not be updated.").with_detail(e.to_string())); }
                Ok(Execution { outcome, report_path: report_path.to_string_lossy().into() })
            }
            Err(e) => { let _ = save_report(&report_path, &session.report, &serde_json::json!({"status":"failed", "error":e.to_string()})); Err(failure(e)) }
        }
    }).await
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "Tauri IPC command parameters are owned by the dispatcher")]
pub fn usb_recovery_cancel(store: State<'_, Arc<RecoveryStore>>, job_id: String) -> AppResult<()> {
    if let Some(cancel) = store
        .state
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?
        .jobs.get(&job_id)
    {
        cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "Tauri IPC command parameters are owned by the dispatcher")]
pub fn usb_recovery_release(
    store: State<'_, Arc<RecoveryStore>>,
    report_id: String,
) -> AppResult<()> {
    store
        .state
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?
        .sessions.remove(&report_id);
    Ok(())
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "Tauri IPC command parameters are owned by the dispatcher")]
pub fn usb_recovery_open_report(app: tauri::AppHandle, report_id: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let path = report_path(&cache(&app)?, &report_id)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::internal(e.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod cache_tests {
    use super::*;

    #[test]
    fn jobs_and_cache_clearing_are_mutually_exclusive() {
        let store = Arc::new(RecoveryStore::default());
        let job = store.job("scanning").unwrap();
        assert!(store.begin_clear().is_err());
        drop(job);
        let clearing = store.begin_clear().unwrap();
        assert!(store.job("repair").is_err());
        assert!(store.begin_clear().is_err());
        drop(clearing);
        assert!(store.job("next-diagnosis").is_ok());
    }

    #[test]
    fn a_preview_protects_its_session_and_clear_invalidates_old_reports() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let session = Session::diagnose(root.path(), cache.path(), &|| false, &mut |_, _, _| {}).unwrap();
        let store = Arc::new(RecoveryStore::default());
        store.state.lock().unwrap().sessions.insert("report".into(), Arc::new(session));
        let (_session, job) = store.session_job("report", "preview").unwrap();
        assert!(store.begin_clear().is_err());
        drop(job);
        let clearing = store.begin_clear().unwrap();
        assert!(store.session_job("report", "late-execute").is_err());
        drop(clearing);
        assert!(store.state.lock().unwrap().sessions.is_empty());
    }
}
