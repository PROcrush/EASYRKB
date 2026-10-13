//! USB diagnosis, backed-up database repair and recovery to a new drive or XML.
//! Diagnosis opens databases only from local copies; repair publishes a verified write set.
use crate::device_library::{self, Format, Library, Track};
use crate::{SourcePlaylist, SourceTrack};
use rbl_anlz::{Anlz, Beat, CueEntry};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

const DATABASE_LIMIT: u64 = 64 * 1024 * 1024;
const ASSET_LIMIT: u64 = 32 * 1024 * 1024;
pub const DEFAULT_SNAPSHOT_LIMIT: u64 = 8 * 1024 * 1024 * 1024;
const MAX_SNAPSHOT_LIMIT: u64 = 64 * 1024 * 1024 * 1024;
const TRACK_LIMIT: usize = 100_000;
type Result<T> = std::result::Result<T, RecoveryError>;

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("Recovery stopped.")]
    Cancelled,
    #[error("The source changed. Diagnose it again before recovering.")]
    Changed,
    #[error("Choose a readable recovery source.")]
    Source,
    #[error("Choose an empty destination outside the source drive.")]
    Destination,
    #[error("The recovery snapshot exceeds the supported size limit.")]
    Limit,
    #[error("The diagnosis cache limit was reached. Increase it and diagnose again.")]
    SnapshotLimit,
    #[error("The device databases already match. Missing files require a backup or relocation.")]
    Healthy,
    #[error("An interrupted publication must be resolved before repairing this drive.")]
    PendingPublication,
    #[error("The recovery output failed verification: {0}")]
    Verification(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Export(#[from] crate::ExportError),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    pub format: String,
    pub database: String,
    pub sha256: String,
    pub status: String,
    pub tracks: usize,
    pub playlists: usize,
    pub playlist_entries: usize,
    pub missing_audio: usize,
    pub analysis_tracks: usize,
    pub cue_tracks: usize,
    pub beat_grid_tracks: usize,
    pub required_bytes: u64,
    pub issues: Vec<Issue>,
}
impl SourceSummary {
    fn issue(&mut self, code: &str, detail: impl Into<String>) {
        if self.issues.len() < 100 {
            self.issues.push(Issue {
                code: code.into(),
                detail: detail.into(),
            });
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub source_path: String,
    pub sources: Vec<SourceSummary>,
    pub pending_publication: bool,
    pub source_disagreement: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub total: usize,
    pub offset: usize,
    pub tracks: Vec<PreviewTrack>,
    pub playlists: Vec<PreviewPlaylist>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTrack {
    pub id: u32,
    pub title: String,
    pub artist: String,
    pub path: String,
    pub audio_present: bool,
    pub analysis_files: usize,
    pub cues: usize,
    pub beats: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPlaylist {
    pub id: u32,
    pub parent: u32,
    pub name: String,
    pub folder: bool,
    pub entries: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub destination: String,
    pub kind: String,
    pub tracks: usize,
    pub playlists: usize,
    pub skipped_audio: usize,
    pub verified: bool,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: u128,
    identity: (u64, u64),
}
fn stamp(path: &Path) -> std::io::Result<Option<Stamp>> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(Some(Stamp {
            size: m.len(),
            modified: m
                .modified()?
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            identity: identity(&m),
        })),
        Ok(_) => Err(std::io::Error::other("Not a regular file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
#[cfg(unix)]
fn identity(m: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino())
}
#[cfg(not(unix))]
fn identity(_m: &fs::Metadata) -> (u64, u64) {
    (0, 0)
}
struct Fingerprint {
    path: PathBuf,
    stamp: Option<Stamp>,
    hash: Option<String>,
}
struct RecoveredTrack {
    track: Track,
    audio: PathBuf,
    present: bool,
    analysis: Vec<(String, PathBuf)>,
    artwork: Option<PathBuf>,
    cues: Vec<CueEntry>,
    beats: Vec<Beat>,
}
struct Source {
    summary: SourceSummary,
    library: Option<Library>,
    tracks: Vec<RecoveredTrack>,
    files: Vec<Fingerprint>,
}
pub struct Session {
    root: PathBuf,
    identity: (u64, u64),
    snapshot: tempfile::TempDir,
    sources: Vec<Source>,
    pub report: Report,
}
fn check(cancel: &dyn Fn() -> bool) -> Result<()> {
    if cancel() {
        Err(RecoveryError::Cancelled)
    } else {
        Ok(())
    }
}

/// Resolve a volume-relative path without following any symlink, including parents.
fn under(root: &Path, value: &str) -> Result<PathBuf> {
    let value = value.replace('\\', "/");
    if value.contains(':') || value.contains('\0') {
        return Err(RecoveryError::Destination);
    }
    let relative = Path::new(value.trim_start_matches('/'));
    if relative.as_os_str().is_empty() {
        return Err(RecoveryError::Destination);
    }
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(RecoveryError::Destination);
        };
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_symlink() => return Err(RecoveryError::Destination),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(path)
}
fn hash_file(path: &Path, cancel: &dyn Fn() -> bool) -> Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        check(cancel)?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn copy_snapshot(
    path: &Path,
    output: &Path,
    limit: u64,
    budget: &mut SnapshotBudget,
    files: &mut Vec<Fingerprint>,
    cancel: &dyn Fn() -> bool,
) -> Result<Option<String>> {
    let before = stamp(path)?;
    let Some(ref metadata) = before else {
        files.push(Fingerprint {
            path: path.into(),
            stamp: None,
            hash: None,
        });
        return Ok(None);
    };
    if metadata.size > limit {
        return Err(RecoveryError::Limit);
    }
    if budget.used.saturating_add(metadata.size) > budget.limit {
        return Err(RecoveryError::SnapshotLimit);
    }
    fs::create_dir_all(output.parent().ok_or(RecoveryError::Destination)?)?;
    let mut input = File::open(path)?;
    let mut output_file = File::create(output)?;
    let mut hash = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        check(cancel)?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        if copied > limit {
            return Err(RecoveryError::Limit);
        }
        if budget.used.saturating_add(copied) > budget.limit {
            return Err(RecoveryError::SnapshotLimit);
        }
        output_file.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
    }
    if before != stamp(path)? || copied != metadata.size {
        return Err(RecoveryError::Changed);
    }
    let hash = format!("{:x}", hash.finalize());
    budget.used += copied;
    files.push(Fingerprint {
        path: path.into(),
        stamp: before,
        hash: Some(hash.clone()),
    });
    Ok(Some(hash))
}

struct SnapshotBudget {
    used: u64,
    limit: u64,
}

fn local_directory(source: &Path, directory: &Path) -> Result<PathBuf> {
    let absolute = if directory.is_absolute() {
        directory.to_owned()
    } else {
        std::env::current_dir()?.join(directory)
    };
    if absolute.starts_with(source) {
        return Err(RecoveryError::Destination);
    }
    // Check the existing ancestor before mkdir: a symlink into the USB must
    // not create a new cache/backup folder there, even if the leaf is absent.
    let mut ancestor = absolute.as_path();
    while !ancestor.try_exists()? {
        ancestor = ancestor.parent().ok_or(RecoveryError::Destination)?;
    }
    if ancestor.canonicalize()?.starts_with(source) {
        return Err(RecoveryError::Destination);
    }
    fs::create_dir_all(&absolute)?;
    let resolved = absolute.canonicalize()?;
    if resolved.starts_with(source) {
        return Err(RecoveryError::Destination);
    }
    Ok(resolved)
}

impl Session {
    #[allow(
        clippy::too_many_lines,
        reason = "one independent snapshot and diagnostic pass per database format"
    )]
    pub fn diagnose(
        root: &Path,
        cache: &Path,
        cancel: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<Self> {
        Self::diagnose_with_limit(root, cache, DEFAULT_SNAPSHOT_LIMIT, cancel, progress)
    }

    /// The limit is per diagnosis, shared by all source database formats.
    #[allow(clippy::too_many_lines)]
    pub fn diagnose_with_limit(
        root: &Path,
        cache: &Path,
        snapshot_limit: u64,
        cancel: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<Self> {
        if snapshot_limit == 0 || snapshot_limit > MAX_SNAPSHOT_LIMIT {
            return Err(RecoveryError::Limit);
        }
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(RecoveryError::Source);
        }
        let cache = local_directory(&root, cache)?;
        let snapshot = tempfile::Builder::new()
            .prefix("usb-recovery-")
            .tempdir_in(&cache)?;
        // A cache inside the source would violate the read-only contract.
        if snapshot.path().canonicalize()?.starts_with(&root) {
            return Err(RecoveryError::Destination);
        }
        let root_identity = identity(&fs::metadata(&root)?);
        let mut sources = Vec::new();
        let mut budget = SnapshotBudget { used: 0, limit: snapshot_limit };
        for name in ["PIONEER", ".PIONEER"] {
            for format in [Format::DeviceLibrary, Format::OneLibrary] {
                check(cancel)?;
                let relative = format!("{name}/rekordbox/{}", format.file_name());
                let Ok(source_path) = under(&root, &relative) else {
                    continue;
                };
                if !source_path.exists() {
                    continue;
                }
                let id = format!(
                    "{}-{}",
                    if name == "PIONEER" {
                        "standard"
                    } else {
                        "hidden"
                    },
                    if format == Format::DeviceLibrary {
                        "pdb"
                    } else {
                        "one"
                    }
                );
                let local_root = snapshot.path().join(&id);
                let local_db = local_root.join(&relative);
                let mut source = Source {
                    summary: SourceSummary {
                        id,
                        format: if format == Format::DeviceLibrary {
                            "Device Library"
                        } else {
                            "OneLibrary"
                        }
                        .into(),
                        database: relative.clone(),
                        sha256: String::new(),
                        status: "unreadable".into(),
                        tracks: 0,
                        playlists: 0,
                        playlist_entries: 0,
                        missing_audio: 0,
                        analysis_tracks: 0,
                        cue_tracks: 0,
                        beat_grid_tracks: 0,
                        required_bytes: 0,
                        issues: Vec::new(),
                    },
                    library: None,
                    tracks: Vec::new(),
                    files: Vec::new(),
                };
                let parsed = (|| -> Result<Library> {
                    source.summary.sha256 = copy_snapshot(
                        &source_path,
                        &local_db,
                        DATABASE_LIMIT,
                        &mut budget,
                        &mut source.files,
                        cancel,
                    )?
                    .ok_or(RecoveryError::Changed)?;
                    if format == Format::OneLibrary {
                        for suffix in ["-wal", "-shm", "-journal"] {
                            let sidecar = under(&root, &(relative.clone() + suffix))?;
                            if copy_snapshot(
                                &sidecar,
                                &local_root.join(relative.clone() + suffix),
                                DATABASE_LIMIT,
                                &mut budget,
                                &mut source.files,
                                cancel,
                            )?
                            .is_some()
                            {
                                source.summary.issue("sidecar", suffix);
                            }
                        }
                        let db = rbl_onelibrary::ExportLibrary::open_read_only(&local_db)
                            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
                        let integrity: String = db
                            .connection()
                            .pragma_query_value(None, "quick_check", |row| row.get(0))
                            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
                        if integrity != "ok" {
                            return Err(RecoveryError::Verification(integrity));
                        }
                    } else {
                        let bytes = fs::read(&local_db)?;
                        let pdb = rbl_pdb::Pdb::parse(&bytes)
                            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
                        for issue in pdb.diagnostics() {
                            source.summary.issue("databaseStructure", issue);
                        }
                    }
                    device_library::read_unlocked(&local_root, format).map_err(Into::into)
                })();
                let mut library = match parsed {
                    Ok(library) => library,
                    Err(RecoveryError::Cancelled) => return Err(RecoveryError::Cancelled),
                    Err(RecoveryError::Changed) => return Err(RecoveryError::Changed),
                    Err(RecoveryError::SnapshotLimit) => {
                        source.summary.issue("cacheLimit", source_path.to_string_lossy());
                        sources.push(source);
                        continue;
                    }
                    Err(e) => {
                        source.summary.issue("unreadable", e.to_string());
                        sources.push(source);
                        continue;
                    }
                };
                if library.tracks.len() > TRACK_LIMIT || library.nodes.len() > TRACK_LIMIT {
                    return Err(RecoveryError::Limit);
                }
                let mut ids = BTreeSet::new();
                for (index, track) in library.tracks.iter().enumerate() {
                    check(cancel)?;
                    progress("scanning", index, library.tracks.len());
                    if track.id == 0 || !ids.insert(track.id) {
                        source.summary.issue("invalidRecord", track.id.to_string());
                        continue;
                    }
                    let audio = match under(&root, &track.path) {
                        Ok(p) => p,
                        Err(e) => {
                            source
                                .summary
                                .issue("unsafePath", format!("{}: {e}", track.path));
                            continue;
                        }
                    };
                    let audio_stamp = match stamp(&audio) {
                        Ok(s) => s,
                        Err(e) => {
                            source
                                .summary
                                .issue("missingAudio", format!("{}: {e}", track.path));
                            None
                        }
                    };
                    let present = audio_stamp.is_some();
                    source.summary.required_bytes += audio_stamp.as_ref().map_or(0, |s| s.size);
                    // Unreadable audio is missing, but not a fatal error for XML recovery.
                    if stamp(&audio).is_ok() {
                        source.files.push(Fingerprint {
                            path: audio.clone(),
                            stamp: audio_stamp,
                            hash: None,
                        });
                    }
                    let mut recovered = RecoveredTrack {
                        track: track.clone(),
                        audio,
                        present,
                        analysis: Vec::new(),
                        artwork: None,
                        cues: Vec::new(),
                        beats: Vec::new(),
                    };
                    let mut cue_map = BTreeMap::new();
                    if !track.analysis_path.is_empty() {
                        match under(&root, &track.analysis_path) {
                            Ok(path) => {
                                for extension in ["DAT", "EXT", "2EX"] {
                                    let path = path.with_extension(extension);
                                    let local = snapshot.path().join(format!(
                                        "assets/{}/{index}.{extension}",
                                        source.summary.id
                                    ));
                                    match copy_snapshot(
                                        &path,
                                        &local,
                                        ASSET_LIMIT,
                                        &mut budget,
                                        &mut source.files,
                                        cancel,
                                    ) {
                                        Ok(Some(_)) => match Anlz::read(&local) {
                                            Ok(analysis) => {
                                                if recovered.beats.is_empty() {
                                                    let offset = i64::from(
                                                        analysis.grid_offset().unwrap_or(0),
                                                    );
                                                    recovered.beats = analysis
                                                        .beat_grid()
                                                        .unwrap_or_default()
                                                        .into_iter()
                                                        .filter_map(|beat| {
                                                            u32::try_from(
                                                                i64::from(beat.time_ms) + offset,
                                                            )
                                                            .ok()
                                                            .map(|time_ms| Beat { time_ms, ..beat })
                                                        })
                                                        .collect();
                                                }
                                                for cue in legacy_cues(&analysis)
                                                    .into_iter()
                                                    .chain(analysis.cue_entries())
                                                {
                                                    if matches!(cue.kind, 1 | 2)
                                                        && cue.hot_cue <= 16
                                                        && (cue.kind != 2
                                                            || cue.loop_time_ms >= cue.time_ms)
                                                    {
                                                        cue_map.insert(
                                                            (
                                                                cue.hot_cue,
                                                                cue.kind,
                                                                cue.time_ms,
                                                                cue.loop_time_ms,
                                                            ),
                                                            cue,
                                                        );
                                                    }
                                                }
                                                recovered.analysis.push((extension.into(), local));
                                            }
                                            Err(e) => source.summary.issue(
                                                "analysisUnreadable",
                                                format!("{}: {e}", path.display()),
                                            ),
                                        },
                                        Ok(None) => {}
                                        Err(RecoveryError::Cancelled) => {
                                            return Err(RecoveryError::Cancelled)
                                        }
                                        Err(RecoveryError::Changed) => {
                                            return Err(RecoveryError::Changed)
                                        }
                                        Err(RecoveryError::SnapshotLimit) => source.summary.issue(
                                            "cacheLimit", path.to_string_lossy(),
                                        ),
                                        Err(e) => source.summary.issue(
                                            "analysisUnreadable",
                                            format!("{}: {e}", path.display()),
                                        ),
                                    }
                                }
                            }
                            Err(_) => source
                                .summary
                                .issue("unsafePath", track.analysis_path.clone()),
                        }
                    }
                    recovered.cues = cue_map.into_values().collect();
                    if recovered.analysis.is_empty() {
                        source.summary.issue("analysisMissing", track.path.clone());
                    }
                    if !track.artwork_path.is_empty() {
                        match under(&root, &track.artwork_path) {
                            Ok(path) => {
                                let local = snapshot
                                    .path()
                                    .join(format!("artwork/{}/{index}.jpg", source.summary.id));
                                match copy_snapshot(
                                    &path,
                                    &local,
                                    ASSET_LIMIT,
                                    &mut budget,
                                    &mut source.files,
                                    cancel,
                                ) {
                                    Ok(Some(_)) => recovered.artwork = Some(local),
                                    Ok(None) => source
                                        .summary
                                        .issue("artworkMissing", track.artwork_path.clone()),
                                    Err(RecoveryError::Cancelled) => {
                                        return Err(RecoveryError::Cancelled)
                                    }
                                    Err(RecoveryError::Changed) => {
                                        return Err(RecoveryError::Changed)
                                    }
                                    Err(RecoveryError::SnapshotLimit) => source.summary.issue(
                                        "cacheLimit", path.to_string_lossy(),
                                    ),
                                    Err(e) => source.summary.issue("artworkMissing", e.to_string()),
                                }
                            }
                            Err(_) => source
                                .summary
                                .issue("unsafePath", track.artwork_path.clone()),
                        }
                    }
                    if !present {
                        source.summary.missing_audio += 1;
                    }
                    if !recovered.analysis.is_empty() {
                        source.summary.analysis_tracks += 1;
                    }
                    if !recovered.cues.is_empty() {
                        source.summary.cue_tracks += 1;
                    }
                    if !recovered.beats.is_empty() {
                        source.summary.beat_grid_tracks += 1;
                    }
                    source.tracks.push(recovered);
                }
                let valid_ids: BTreeSet<_> = source.tracks.iter().map(|t| t.track.id).collect();
                let mut node_ids = BTreeSet::new();
                library.nodes.retain(|n| n.id != 0 && node_ids.insert(n.id));
                let parents: BTreeMap<_, _> = library
                    .nodes
                    .iter()
                    .map(|n| (n.id, (n.parent, n.folder)))
                    .collect();
                for node in &mut library.nodes {
                    let mut visited = BTreeSet::from([node.id]);
                    let mut parent = node.parent;
                    while parent != 0 {
                        if !visited.insert(parent)
                            || !parents.get(&parent).is_some_and(|(_, folder)| *folder)
                        {
                            source.summary.issue("playlistHierarchy", node.name.clone());
                            node.parent = 0;
                            break;
                        }
                        parent = parents.get(&parent).map_or(0, |(parent, _)| *parent);
                    }
                }
                for node in &mut library.nodes {
                    let original = node.tracks.len();
                    node.tracks.retain(|id| valid_ids.contains(id));
                    if original != node.tracks.len() {
                        source.summary.issue(
                            "playlistMembersMissing",
                            format!("{}: {}", node.name, original - node.tracks.len()),
                        );
                    }
                }
                source.summary.tracks = source.tracks.len();
                source.summary.playlists = library.nodes.iter().filter(|n| !n.folder).count();
                source.summary.playlist_entries =
                    library.nodes.iter().map(|n| n.tracks.len()).sum();
                source.summary.required_bytes = source
                    .summary
                    .required_bytes
                    .saturating_mul(2)
                    .saturating_add(budget.used.saturating_mul(2))
                    .saturating_add(32 * 1024 * 1024);
                source.summary.status =
                    if source.summary.issues.iter().any(|i| i.code != "sidecar") {
                        "partial"
                    } else {
                        "readable"
                    }
                    .into();
                source.library = Some(library);
                sources.push(source);
            }
        }
        let signatures: BTreeSet<_> = sources
            .iter()
            .filter_map(|s| s.library.as_ref().map(content_signature))
            .collect();
        let report = Report {
            source_path: root.to_string_lossy().into(),
            sources: sources.iter().map(|s| s.summary.clone()).collect(),
            pending_publication: root.join(".rbxport-publication").exists(),
            source_disagreement: signatures.len() > 1,
        };
        let session = Self {
            root,
            identity: root_identity,
            snapshot,
            sources,
            report,
        };
        for source in &session.sources {
            if source.library.is_some() {
                session.validate(source, cancel)?;
            }
        }
        progress("scanning", 1, 1);
        Ok(session)
    }

    fn source(&self, id: &str) -> Result<&Source> {
        self.sources
            .iter()
            .find(|s| s.summary.id == id && s.library.is_some())
            .ok_or(RecoveryError::Source)
    }
    fn validate(&self, source: &Source, cancel: &dyn Fn() -> bool) -> Result<()> {
        if identity(&fs::metadata(&self.root).map_err(|_| RecoveryError::Changed)?) != self.identity
        {
            return Err(RecoveryError::Changed);
        }
        for file in &source.files {
            check(cancel)?;
            let relative = file
                .path
                .strip_prefix(&self.root)
                .map_err(|_| RecoveryError::Changed)?;
            let safe = under(&self.root, &relative.to_string_lossy())
                .map_err(|_| RecoveryError::Changed)?;
            if stamp(&safe).map_err(|_| RecoveryError::Changed)? != file.stamp {
                return Err(RecoveryError::Changed);
            }
            if let Some(hash) = &file.hash {
                if &hash_file(&safe, cancel)? != hash {
                    return Err(RecoveryError::Changed);
                }
            }
        }
        Ok(())
    }
    pub fn preview(&self, id: &str, offset: usize, limit: usize) -> Result<Preview> {
        let source = self.source(id)?;
        Ok(Preview {
            total: source.tracks.len(),
            offset,
            tracks: source
                .tracks
                .iter()
                .skip(offset)
                .take(limit.clamp(1, 100))
                .map(|t| PreviewTrack {
                    id: t.track.id,
                    title: t.track.title.clone(),
                    artist: t.track.artist.clone(),
                    path: t.track.path.clone(),
                    audio_present: t.present,
                    analysis_files: t.analysis.len(),
                    cues: t.cues.len(),
                    beats: t.beats.len(),
                })
                .collect(),
            playlists: source
                .library
                .as_ref()
                .ok_or(RecoveryError::Source)?
                .nodes
                .iter()
                .take(1000)
                .map(|n| PreviewPlaylist {
                    id: n.id,
                    parent: n.parent,
                    name: n.name.clone(),
                    folder: n.folder,
                    entries: n.tracks.len(),
                })
                .collect(),
        })
    }
    fn outside_source(&self, target: &Path) -> Result<PathBuf> {
        let target = target.canonicalize()?;
        if target.starts_with(&self.root) || self.root.starts_with(&target) {
            return Err(RecoveryError::Destination);
        }
        Ok(target)
    }
    pub fn restore_to_empty(
        &self,
        id: &str,
        target: &Path,
        cancel: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<Outcome> {
        let source = self.source(id)?;
        self.validate(source, cancel)?;
        let target = self.outside_source(target)?;
        empty_target(&target, None)?;
        let publication = rbl_core::durable::Publication::new(&target, ".rbxport-publication")?;
        empty_target(&target, Some(publication.stage()))?;
        let mut tracks = Vec::new();
        for recovered in source.tracks.iter().filter(|t| t.present) {
            check(cancel)?;
            let t = &recovered.track;
            tracks.push(SourceTrack {
                id: u64::from(t.id),
                source_path: recovered.audio.clone(),
                title: t.title.clone(),
                artist: t.artist.clone(),
                album: t.album.clone(),
                genre: t.genre.clone(),
                label: t.label.clone(),
                key: t.key.clone(),
                comment: t.comment.clone(),
                date_added: t.date_added.clone(),
                bpm_x100: t.bpm_x100,
                duration_sec: u16::try_from(t.duration_sec).unwrap_or(u16::MAX),
                rating: t.rating,
                color_id: t.color,
                analysis: recovered
                    .analysis
                    .iter()
                    .map(|(extension, path)| Ok((extension.clone(), fs::read(path)?)))
                    .collect::<std::io::Result<_>>()?,
                artwork: recovered.artwork.clone(),
                ..Default::default()
            });
        }
        let indices: BTreeMap<_, _> = tracks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| u32::try_from(t.id).ok().map(|id| (id, i)))
            .collect();
        let playlists: Vec<_> = source
            .library
            .as_ref()
            .ok_or(RecoveryError::Source)?
            .nodes
            .iter()
            .map(|n| SourcePlaylist {
                id: u64::from(n.id),
                name: n.name.clone(),
                parent_id: u64::from(n.parent),
                folder: n.folder,
                track_indices: n
                    .tracks
                    .iter()
                    .filter_map(|id| indices.get(id).copied())
                    .collect(),
                ..Default::default()
            })
            .collect();
        let result = crate::export_cancellable(
            publication.stage(),
            &tracks,
            &playlists,
            &[],
            &crate::ExportOptions::default(),
            &mut |p| progress(p.stage, p.done, p.total),
            cancel,
        )?;
        if !result.skipped.is_empty() || !result.failed.is_empty() || result.tracks != tracks.len()
        {
            return Err(RecoveryError::Verification(
                result
                    .failed
                    .into_iter()
                    .chain(result.skipped)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        let staged = crate::verify_databases(publication.stage())?;
        if !staged.is_ok() {
            return Err(RecoveryError::Verification(staged.errors.join("; ")));
        }
        let mut paths = Vec::new();
        generated_files(publication.stage(), publication.stage(), &mut paths)?;
        self.validate(source, cancel)?;
        check(cancel)?;
        // Recheck immediately before committing, so intervening user files are never overwritten.
        empty_target(&target, Some(publication.stage()))?;
        progress("publishing", 0, 1);
        publication.commit(&paths)?;
        let verified = crate::verify_databases(&target)?;
        if !verified.is_ok() {
            return Err(RecoveryError::Verification(verified.errors.join("; ")));
        }
        progress("complete", 1, 1);
        Ok(Outcome {
            destination: target.to_string_lossy().into(),
            kind: "usb".into(),
            tracks: result.tracks,
            playlists: result.playlists,
            skipped_audio: source.summary.missing_audio,
            verified: true,
            backup_path: None,
        })
    }
    pub fn write_xml(
        &self,
        id: &str,
        target: &Path,
        cancel: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<Outcome> {
        let source = self.source(id)?;
        self.validate(source, cancel)?;
        let parent = self.outside_source(target.parent().ok_or(RecoveryError::Destination)?)?;
        // persist_noclobber prevents overwriting an existing XML or following a destination symlink.
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        write!(file, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DJ_PLAYLISTS Version=\"1.0.0\"><PRODUCT Name=\"EASYRKB\" Version=\"1.6.0\" Company=\"EASYRKB\"/><COLLECTION Entries=\"{}\">", source.tracks.len())?;
        for (index, t) in source.tracks.iter().enumerate() {
            check(cancel)?;
            progress("xml", index, source.tracks.len());
            write!(file, "<TRACK TrackID=\"{}\" Name=\"{}\" Artist=\"{}\" Album=\"{}\" Genre=\"{}\" Label=\"{}\" Tonality=\"{}\" Comments=\"{}\" DateAdded=\"{}\" AverageBpm=\"{:.2}\" TotalTime=\"{}\" Rating=\"{}\" Location=\"{}\">", t.track.id, xml(&t.track.title), xml(&t.track.artist), xml(&t.track.album), xml(&t.track.genre), xml(&t.track.label), xml(&t.track.key), xml(&t.track.comment), xml(&t.track.date_added), f64::from(t.track.bpm_x100) / 100., t.track.duration_sec, t.track.rating * 51, xml(&file_url(&t.audio)))?;
            for beat in &t.beats {
                write!(
                    file,
                    "<TEMPO Inizio=\"{:.3}\" Bpm=\"{:.2}\" Metro=\"4/4\" Battito=\"{}\"/>",
                    f64::from(beat.time_ms) / 1000.,
                    f64::from(beat.tempo_x100) / 100.,
                    beat.beat_number
                )?;
            }
            for cue in &t.cues {
                write!(
                    file,
                    "<POSITION_MARK Name=\"{}\" Type=\"{}\" Start=\"{:.3}\" Num=\"{}\"",
                    xml(cue.comment.as_deref().unwrap_or("")),
                    if cue.kind == 2 { 4 } else { 0 },
                    f64::from(cue.time_ms) / 1000.,
                    i64::from(cue.hot_cue) - 1
                )?;
                if cue.kind == 2 {
                    write!(file, " End=\"{:.3}\"", f64::from(cue.loop_time_ms) / 1000.)?;
                }
                if let Some([r, g, b]) = cue.rgb {
                    write!(file, " Red=\"{r}\" Green=\"{g}\" Blue=\"{b}\"")?;
                }
                write!(file, "/>")?;
            }
            write!(file, "</TRACK>")?;
        }
        let nodes = &source.library.as_ref().ok_or(RecoveryError::Source)?.nodes;
        write!(
            file,
            "</COLLECTION><PLAYLISTS><NODE Type=\"0\" Name=\"ROOT\" Count=\"{}\">",
            nodes.iter().filter(|n| n.parent == 0).count()
        )?;
        let mut seen = BTreeSet::new();
        write_nodes(&mut file, nodes, 0, &mut seen, 0)?;
        if seen.len() != nodes.len() {
            return Err(RecoveryError::Verification("Unreachable playlists".into()));
        }
        writeln!(file, "</NODE></PLAYLISTS></DJ_PLAYLISTS>")?;
        file.as_file().sync_all()?;
        self.validate(source, cancel)?;
        check(cancel)?;
        file.persist_noclobber(target)
            .map_err(|e| RecoveryError::Io(e.error))?;
        progress("complete", 1, 1);
        Ok(Outcome {
            destination: target.to_string_lossy().into(),
            kind: "xml".into(),
            tracks: source.tracks.len(),
            playlists: source.summary.playlists,
            skipped_audio: source.summary.missing_audio,
            verified: true,
            backup_path: None,
        })
    }
}

impl Session {
    /// Rebuild the other database format in place. Audio, ANLZ, the selected
    /// good database and unrelated files are never included in the write set.
    #[allow(
        clippy::too_many_lines,
        reason = "one ordered backup, validation and publication transaction"
    )]
    pub fn repair_current(
        &self,
        id: &str,
        backups: &Path,
        cancel: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<Outcome> {
        let source = self.source(id)?;
        self.validate(source, cancel)?;
        if self.root.join(".rbxport-publication").exists() {
            return Err(RecoveryError::PendingPublication);
        }
        let backups = local_directory(&self.root, backups)?;
        let library = source.library.as_ref().ok_or(RecoveryError::Source)?;
        let root_name = source
            .summary
            .database
            .split('/')
            .next()
            .ok_or(RecoveryError::Source)?;
        let target_format = if library.format == Format::DeviceLibrary {
            Format::OneLibrary
        } else {
            Format::DeviceLibrary
        };
        if self.sources.iter().any(|other| {
            other.summary.database == format!("{root_name}/rekordbox/{}", target_format.file_name())
                && other
                    .library
                    .as_ref()
                    .is_some_and(|l| content_signature(l) == content_signature(library))
                && !other
                    .summary
                    .issues
                    .iter()
                    .any(|i| i.code == "databaseStructure")
        }) {
            return Err(RecoveryError::Healthy);
        }
        let relative = PathBuf::from(root_name)
            .join("rekordbox")
            .join(target_format.file_name());
        let mut changed = vec![relative.clone()];
        if target_format == Format::OneLibrary {
            // Old sidecars belong to the replaced database and must not replay.
            changed.splice(
                0..0,
                ["-wal", "-shm", "-journal"]
                    .map(|suffix| PathBuf::from(relative.to_string_lossy().into_owned() + suffix)),
            );
        }
        // Back up both database generations and sidecars, including absent-file
        // records. A failed backup aborts before creating anything on the USB.
        progress("backup", 0, 1);
        let backup = tempfile::Builder::new()
            .prefix("EASYRKB-before-repair-")
            .tempdir_in(&backups)?;
        let mut fingerprints = Vec::new();
        let mut budget = SnapshotBudget { used: 0, limit: DEFAULT_SNAPSHOT_LIMIT };
        let mut manifest = Vec::new();
        for format in [Format::DeviceLibrary, Format::OneLibrary] {
            for suffix in ["", "-wal", "-shm", "-journal"] {
                let path = format!("{root_name}/rekordbox/{}{suffix}", format.file_name());
                let original = under(&self.root, &path)?;
                let copied = backup.path().join(&path);
                let hash = copy_snapshot(
                    &original,
                    &copied,
                    DATABASE_LIMIT,
                    &mut budget,
                    &mut fingerprints,
                    cancel,
                )?;
                if let Some(ref hash) = hash {
                    if &hash_file(&copied, cancel)? != hash {
                        return Err(RecoveryError::Verification("Backup hash mismatch".into()));
                    }
                    File::open(&copied)?.sync_all()?;
                    if let Some(parent) = copied.parent() {
                        rbl_core::durable::sync_dir(parent)?;
                    }
                }
                manifest
                    .push(serde_json::json!({"path":path,"sha256":hash,"present":hash.is_some()}));
            }
        }
        self.validate(source, cancel)?;
        validate_files(&self.root, &fingerprints, cancel)?;
        let manifest_path = backup.path().join("backup.json");
        fs::write(&manifest_path, serde_json::to_vec_pretty(&serde_json::json!({"app":"EASYRKB","source":self.report,"selectedSource":id,"changes":changed,"files":manifest})).map_err(|e| std::io::Error::other(e.to_string()))?)?;
        File::open(manifest_path)?.sync_all()?;
        rbl_core::durable::sync_dir(&backup.path().join(root_name))?;
        rbl_core::durable::sync_dir(backup.path())?;
        let backup_path = backup.keep();
        rbl_core::durable::sync_dir(&backups)?;
        // Generation and structural checks happen on the computer first.
        let generated = tempfile::Builder::new()
            .prefix("EASYRKB-repair-build-")
            .tempdir_in(&backups)?;
        let selected_relative = PathBuf::from(&source.summary.database);
        fs::create_dir_all(generated.path().join(root_name).join("rekordbox"))?;
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let path = selected_relative.to_string_lossy().into_owned() + suffix;
            if backup_path.join(&path).is_file() {
                fs::copy(backup_path.join(&path), generated.path().join(&path))?;
            }
        }
        let selected_db = self
            .snapshot
            .path()
            .join(&source.summary.id)
            .join(&source.summary.database);
        progress("database", 0, 1);
        build_repair_database(
            &generated.path().join(&relative),
            source,
            &selected_db,
            target_format,
            cancel,
        )?;
        let verify = crate::verify_databases(generated.path())?;
        if !verify.is_ok() {
            return Err(RecoveryError::Verification(format!(
                "{}; backup: {}",
                verify.errors.join("; "),
                backup_path.display()
            )));
        }
        self.validate(source, cancel)?;
        validate_files(&self.root, &fingerprints, cancel)?;
        check(cancel)?;
        // Does not replay any existing journal. The exclusive lock remains held
        // through publication and read-back, blocking other EASYRKB writers.
        let publication =
            rbl_core::durable::Publication::new_clean(&self.root, ".rbxport-publication")?;
        self.validate(source, cancel)?;
        validate_files(&self.root, &fingerprints, cancel)?;
        let staged = publication.stage().join(&relative);
        fs::create_dir_all(staged.parent().ok_or(RecoveryError::Destination)?)?;
        fs::copy(generated.path().join(&relative), &staged)?;
        check(cancel)?;
        progress("publishing", 0, 1);
        publication.commit(&changed).map_err(|e| {
            RecoveryError::Verification(format!("{e}; backup: {}", backup_path.display()))
        })?;
        // Read directly while holding the exclusive lock, without nested locks
        // or ordinary device reads that could replay a publication.
        let selected = device_library::read_unlocked(&self.root, library.format).map_err(|e| {
            RecoveryError::Verification(format!("{e}; backup: {}", backup_path.display()))
        })?;
        let repaired = device_library::read_unlocked(&self.root, target_format).map_err(|e| {
            RecoveryError::Verification(format!("{e}; backup: {}", backup_path.display()))
        })?;
        if content_signature(&selected) != content_signature(&repaired) {
            return Err(RecoveryError::Verification(format!(
                "Repaired database differs from the selected source; backup: {}",
                backup_path.display()
            )));
        }
        progress("complete", 1, 1);
        Ok(Outcome {
            destination: self.root.join(&relative).to_string_lossy().into(),
            kind: "repair".into(),
            tracks: source.tracks.len(),
            playlists: source.summary.playlists,
            skipped_audio: source.summary.missing_audio,
            verified: true,
            backup_path: Some(backup_path.to_string_lossy().into()),
        })
    }
}

fn content_signature(library: &Library) -> String {
    // Both readers already sort siblings. PDB sequences start at 1 whereas
    // OneLibrary may start at 0: compare actual order rather than its encoding.
    let nodes = library
        .nodes
        .iter()
        .map(|n| (n.id, n.parent, &n.name, n.folder, &n.tracks))
        .collect::<Vec<_>>();
    format!("{:?}{nodes:?}", library.tracks)
}

fn validate_files(root: &Path, files: &[Fingerprint], cancel: &dyn Fn() -> bool) -> Result<()> {
    for file in files {
        check(cancel)?;
        let relative = file
            .path
            .strip_prefix(root)
            .map_err(|_| RecoveryError::Changed)?;
        let safe = under(root, &relative.to_string_lossy()).map_err(|_| RecoveryError::Changed)?;
        if stamp(&safe).map_err(|_| RecoveryError::Changed)? != file.stamp {
            return Err(RecoveryError::Changed);
        }
        if let Some(hash) = &file.hash {
            if &hash_file(&safe, cancel)? != hash {
                return Err(RecoveryError::Changed);
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Extra {
    metadata: rbl_core::ExportMetadata,
    size: u64,
    bitrate: u32,
    sample_rate: u32,
    year: u16,
    release_date: String,
}
fn extra_metadata(path: &Path, format: Format) -> Result<BTreeMap<u32, Extra>> {
    if format == Format::DeviceLibrary {
        let bytes = fs::read(path)?;
        let pdb =
            rbl_pdb::Pdb::parse(&bytes).map_err(|e| RecoveryError::Verification(e.to_string()))?;
        Ok(pdb
            .table(rbl_pdb::PageType::Tracks)
            .into_iter()
            .flat_map(|table| pdb.track_rows(table))
            .map(|t| {
                (
                    t.id,
                    Extra {
                        metadata: rbl_core::ExportMetadata {
                            track_number: t.track_number,
                            disc_number: t.disc_number,
                            bit_depth: t.sample_depth,
                            play_count: u32::from(t.play_count),
                            hot_cue_auto_load: t.hot_cue_auto_load,
                            ..Default::default()
                        },
                        size: u64::from(t.file_size),
                        bitrate: t.bitrate,
                        sample_rate: t.sample_rate,
                        year: t.year,
                        release_date: t.release_date,
                    },
                )
            })
            .collect())
    } else {
        let db = rbl_onelibrary::ExportLibrary::open_read_only(path)
            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
        let mut query = db.connection().prepare("SELECT content_id,COALESCE(trackNo,0),COALESCE(discNo,0),COALESCE(bitDepth,0),COALESCE(djPlayCount,0),COALESCE(isHotCueAutoLoadOn,0),COALESCE(fileSize,0),COALESCE(bitrate,0),COALESCE(samplingRate,0),COALESCE(releaseYear,0),COALESCE(releaseDate,''),COALESCE(isrc,''),COALESCE(dateCreated,'') FROM content").map_err(|e| RecoveryError::Verification(e.to_string()))?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    Extra {
                        metadata: rbl_core::ExportMetadata {
                            track_number: r.get(1)?,
                            disc_number: r.get(2)?,
                            bit_depth: r.get(3)?,
                            play_count: r.get(4)?,
                            hot_cue_auto_load: r.get(5)?,
                            isrc: r.get(11)?,
                            date_created: r.get(12)?,
                            ..Default::default()
                        },
                        size: u64::try_from(r.get::<_, i64>(6)?).unwrap_or(0),
                        bitrate: r.get(7)?,
                        sample_rate: r.get(8)?,
                        year: r.get(9)?,
                        release_date: r.get(10)?,
                    },
                ))
            })
            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|e| RecoveryError::Verification(e.to_string()))
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "field mappings for both device database formats"
)]
fn build_repair_database(
    path: &Path,
    source: &Source,
    selected_db: &Path,
    format: Format,
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    use rbl_pdb::rows;
    let library = source.library.as_ref().ok_or(RecoveryError::Source)?;
    let extras = extra_metadata(selected_db, library.format)?;
    let default_extra = Extra::default();
    if format == Format::OneLibrary {
        use rbl_onelibrary::build::{Builder, LookupTable, Track as OneTrack};
        let mut builder =
            Builder::create(path).map_err(|e| RecoveryError::Verification(e.to_string()))?;
        for recovered in &source.tracks {
            check(cancel)?;
            let t = &recovered.track;
            let extra = extras.get(&t.id).unwrap_or(&default_extra);
            let artist = builder
                .intern(LookupTable::Artist, &t.artist)
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            let album = builder
                .intern(LookupTable::Album, &t.album)
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            let genre = builder
                .intern(LookupTable::Genre, &t.genre)
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            let label = builder
                .intern(LookupTable::Label, &t.label)
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            let key = builder
                .intern(LookupTable::Key, &t.key)
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            if !t.artwork_path.is_empty() {
                builder
                    .add_image(i64::from(t.id), &t.artwork_path)
                    .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            }
            builder
                .add_track(&OneTrack {
                    content_id: i64::from(t.id),
                    title: t.title.clone(),
                    artist_id: Some(artist),
                    album_id: Some(album),
                    genre_id: Some(genre),
                    label_id: Some(label),
                    key_id: Some(key),
                    color_id: Some(i64::from(t.color)),
                    bpm_x100: i64::from(t.bpm_x100),
                    length: i64::from(t.duration_sec),
                    track_no: i64::from(extra.metadata.track_number),
                    path: t.path.clone(),
                    file_name: Path::new(&t.path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    file_size: i64::try_from(extra.size).unwrap_or(i64::MAX),
                    analysis_path: t.analysis_path.clone(),
                    rating: i64::from(t.rating) * 51,
                    comment: t.comment.clone(),
                    date_added: t.date_added.clone(),
                    image_id: (!t.artwork_path.is_empty()).then_some(i64::from(t.id)),
                    metadata: extra.metadata.clone(),
                    bitrate: i64::from(extra.bitrate),
                    sample_rate: i64::from(extra.sample_rate),
                    year: i64::from(extra.year),
                    release_date: extra.release_date.clone(),
                    file_type: i64::from(rows::audio_file_type(&t.path)),
                    ..Default::default()
                })
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            let cues = recovered
                .cues
                .iter()
                .map(|cue| rbl_anlz::cues::ExportCue {
                    kind: u8::try_from(if cue.hot_cue > 3 {
                        cue.hot_cue + 1
                    } else {
                        cue.hot_cue
                    })
                    .unwrap_or(0),
                    time_ms: cue.time_ms,
                    loop_time_ms: (cue.kind == 2).then_some(cue.loop_time_ms),
                    comment: cue.comment.clone().unwrap_or_default(),
                    color_id: cue.color_id,
                    color_code: cue.color_code.unwrap_or(0),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            builder
                .replace_cues(i64::from(t.id), &cues)
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
        }
        for node in &library.nodes {
            builder
                .add_playlist_node(
                    i64::from(node.id),
                    &node.name,
                    i64::from(node.parent),
                    i64::from(node.sequence),
                    node.folder,
                )
                .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            for (index, id) in node.tracks.iter().enumerate() {
                builder
                    .add_to_playlist(
                        i64::from(node.id),
                        i64::from(*id),
                        i64::try_from(index + 1).unwrap_or(i64::MAX),
                    )
                    .map_err(|e| RecoveryError::Verification(e.to_string()))?;
            }
        }
        builder
            .finish("", &rbl_core::time::now()[..10], 0)
            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
    } else {
        let mut artists = crate::Intern::default();
        let mut albums = crate::Intern::default();
        let mut genres = crate::Intern::default();
        let mut labels = crate::Intern::default();
        let mut keys = crate::Intern::default();
        let mut tracks = Vec::new();
        let mut artwork = Vec::new();
        for recovered in &source.tracks {
            check(cancel)?;
            let t = &recovered.track;
            let extra = extras.get(&t.id).unwrap_or(&default_extra);
            if !t.artwork_path.is_empty() {
                artwork.push(rows::artwork_row(t.id, &t.artwork_path));
            }
            tracks.push(rows::track_row(&rows::TrackInput {
                id: t.id,
                artist_id: artists.id(&t.artist),
                album_id: albums.id(&t.album),
                genre_id: genres.id(&t.genre),
                label_id: labels.id(&t.label),
                key_id: keys.id(&t.key),
                artwork_id: if t.artwork_path.is_empty() { 0 } else { t.id },
                color_id: t.color,
                rating: t.rating,
                tempo_x100: t.bpm_x100,
                duration_sec: u16::try_from(t.duration_sec).unwrap_or(u16::MAX),
                title: t.title.clone(),
                filename: Path::new(&t.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                file_path: t.path.clone(),
                analyze_path: t.analysis_path.clone(),
                comment: t.comment.clone(),
                date_added: t.date_added.clone(),
                release_date: extra.release_date.clone(),
                year: extra.year,
                bitrate: extra.bitrate,
                sample_rate: extra.sample_rate,
                file_size: u32::try_from(extra.size).unwrap_or(u32::MAX),
                hot_cue_auto_load: extra.metadata.hot_cue_auto_load,
                track_number: extra.metadata.track_number,
                disc_number: extra.metadata.disc_number,
                sample_depth: extra.metadata.bit_depth,
                play_count: u16::try_from(extra.metadata.play_count).unwrap_or(u16::MAX),
                isrc: extra.metadata.isrc.clone(),
                ..Default::default()
            }));
        }
        let playlists = library
            .nodes
            .iter()
            .map(|n| rows::playlist_row(n.id, n.parent, n.sequence, n.folder, &n.name))
            .collect::<Vec<_>>();
        let entries = library
            .nodes
            .iter()
            .flat_map(|n| {
                n.tracks.iter().enumerate().map(move |(i, id)| {
                    rows::playlist_entry_row(u32::try_from(i + 1).unwrap_or(u32::MAX), *id, n.id)
                })
            })
            .collect::<Vec<_>>();
        let settings = rbl_onelibrary::settings::StickSettings::read(selected_db)
            .map_err(|e| RecoveryError::Verification(e.to_string()))?;
        let bytes = crate::build_pdb(&crate::PdbTables {
            tracks: &tracks,
            artists: &artists
                .entries()
                .map(|(id, name)| rows::artist_row(id, name))
                .collect::<Vec<_>>(),
            albums: &albums
                .entries()
                .map(|(id, name)| rows::album_row(id, 0, name))
                .collect::<Vec<_>>(),
            genres: &genres
                .entries()
                .map(|(id, name)| rows::simple_named_row(id, name))
                .collect::<Vec<_>>(),
            labels: &labels
                .entries()
                .map(|(id, name)| rows::simple_named_row(id, name))
                .collect::<Vec<_>>(),
            keys: &keys
                .entries()
                .map(|(id, name)| rows::key_row(id, name))
                .collect::<Vec<_>>(),
            playlists: &playlists,
            entries: &entries,
            artwork: &artwork,
            history: &[],
            colors: &settings.colors,
            device_name: &settings.device_name,
            background: u8::try_from(settings.background_color_type).unwrap_or(0),
        });
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn empty_target(root: &Path, own_stage: Option<&Path>) -> Result<()> {
    if !root.is_dir() {
        return Err(RecoveryError::Destination);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            return Err(RecoveryError::Destination);
        }
        if own_stage == Some(entry.path().as_path()) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if [
            ".DS_Store",
            ".Spotlight-V100",
            ".fseventsd",
            ".Trashes",
            "System Volume Information",
            ".rbxport-write.lock",
        ]
        .contains(&name.as_ref())
        {
            continue;
        }
        return Err(RecoveryError::Destination);
    }
    Ok(())
}
fn generated_files(root: &Path, current: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(RecoveryError::Destination);
        }
        if metadata.is_dir() {
            generated_files(root, &path, output)?;
        } else if entry.file_name() != ".rbxport-write.lock" {
            output.push(
                path.strip_prefix(root)
                    .map_err(|_| RecoveryError::Destination)?
                    .into(),
            );
        }
    }
    Ok(())
}
fn legacy_cues(analysis: &Anlz) -> Vec<CueEntry> {
    let mut cues = Vec::new();
    for section in &analysis.sections {
        if section.tag.as_str() != "PCOB" {
            continue;
        }
        let mut bytes = section.payload.as_slice();
        while bytes.len() >= 56 && &bytes[..4] == b"PCPT" {
            let u32_at =
                |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap_or([0; 4]));
            let length = u32_at(8) as usize;
            if length < 56 || length > bytes.len() {
                break;
            }
            cues.push(CueEntry {
                hot_cue: u32_at(12),
                kind: bytes[28],
                time_ms: u32_at(32),
                loop_time_ms: u32_at(36),
                color_id: 0,
                comment: None,
                color_code: None,
                rgb: None,
            });
            bytes = &bytes[length..];
        }
    }
    cues
}
fn xml(s: &str) -> String {
    s.chars()
        .filter(|c| {
            matches!(c, '\t' | '\n' | '\r')
                || (*c >= '\u{20}' && *c != '\u{fffe}' && *c != '\u{ffff}')
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn file_url(path: &Path) -> String {
    let mut out = "file://localhost".to_string();
    let path = path.to_string_lossy().replace('\\', "/");
    if !path.starts_with('/') {
        out.push('/');
    }
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(b"0123456789ABCDEF"[usize::from(byte >> 4)]));
            out.push(char::from(b"0123456789ABCDEF"[usize::from(byte & 15)]));
        }
    }
    out
}
fn write_nodes(
    file: &mut impl Write,
    nodes: &[device_library::Node],
    parent: u32,
    seen: &mut BTreeSet<u32>,
    depth: usize,
) -> Result<()> {
    if depth > 256 {
        return Err(RecoveryError::Verification(
            "Playlist nesting exceeds the supported limit".into(),
        ));
    }
    for node in nodes.iter().filter(|n| n.parent == parent) {
        if !seen.insert(node.id) {
            return Err(RecoveryError::Verification("Playlist cycle".into()));
        }
        if node.folder {
            write!(
                file,
                "<NODE Type=\"0\" Name=\"{}\" Count=\"{}\">",
                xml(&node.name),
                nodes.iter().filter(|n| n.parent == node.id).count()
            )?;
            write_nodes(file, nodes, node.id, seen, depth + 1)?;
        } else {
            write!(
                file,
                "<NODE Type=\"1\" Name=\"{}\" KeyType=\"0\" Entries=\"{}\">",
                xml(&node.name),
                node.tracks.len()
            )?;
            for id in &node.tracks {
                write!(file, "<TRACK Key=\"{id}\"/>")?;
            }
        }
        write!(file, "</NODE>")?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod cache_tests {
    use super::*;

    #[test]
    fn snapshot_can_pass_the_old_one_gib_limit() {
        let source = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let asset = source.path().join("analysis.EXT");
        fs::write(&asset, [7_u8; 32]).unwrap();
        let mut budget = SnapshotBudget { used: 1024 * 1024 * 1024 - 16, limit: DEFAULT_SNAPSHOT_LIMIT };
        let mut files = Vec::new();
        let result = copy_snapshot(
            &asset,
            &cache.path().join("analysis.EXT"),
            ASSET_LIMIT,
            &mut budget,
            &mut files,
            &|| false,
        );
        assert!(result.is_ok(), "a normal analysis asset must fit above 1 GiB: {result:?}");
        assert_eq!(fs::read(cache.path().join("analysis.EXT")).unwrap(), [7_u8; 32]);
    }

    #[test]
    fn configured_limit_rejects_an_asset_before_creating_its_copy() {
        let source = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let asset = source.path().join("analysis.EXT");
        let copy = cache.path().join("analysis.EXT");
        fs::write(&asset, [7_u8; 32]).unwrap();
        let mut budget = SnapshotBudget { used: 16, limit: 32 };
        let mut files = Vec::new();
        assert!(matches!(copy_snapshot(&asset, &copy, ASSET_LIMIT, &mut budget, &mut files, &|| false), Err(RecoveryError::SnapshotLimit)));
        assert!(!copy.exists());
        assert_eq!(fs::read(asset).unwrap(), [7_u8; 32]);
    }
}
