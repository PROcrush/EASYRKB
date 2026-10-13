//! The restore journal. Every file a restore replaces is renamed aside and
//! its replacement renamed in, with this journal saved first; if either
//! program stops midway, the next to start rolls an uncommitted restore back
//! and finishes cleaning up a committed one.
use crate::{refused, sidecar, Result, LIBRARY_FILES};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const NAME: &str = "backup-restore.json";

/// Every staged and set-aside path starts with this, beside its target.
pub const PREFIX: &str = ".rbxport-restore-";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Swap {
    pub target: PathBuf,
    /// The replacement, until it is renamed into place. Absent when the
    /// restore removes the target, as with a stale WAL.
    pub staged: PathBuf,
    /// The original, once it is renamed aside.
    pub previous: PathBuf,
    pub existed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub library: PathBuf,
    pub committed: bool,
    pub swaps: Vec<Swap>,
}

pub fn path(root: &Path) -> PathBuf {
    root.join(NAME)
}

pub fn pending(root: &Path) -> bool {
    path(root).exists()
}

pub(crate) fn save(root: &Path, journal: &Journal) -> Result<()> {
    rbl_core::durable::create_dir_all(root)?;
    Ok(rbl_core::durable::write(&path(root), &serde_json::to_vec(journal)?)?)
}

/// A fresh pair of staged and set-aside paths beside `target`.
pub(crate) fn swap_for(target: &Path) -> Result<Swap> {
    let parent = target.parent().ok_or_else(|| refused("Invalid restore path"))?;
    let id = uuid::Uuid::new_v4();
    Ok(Swap {
        target: target.to_path_buf(),
        staged: parent.join(format!("{PREFIX}{id}-new")),
        previous: parent.join(format!("{PREFIX}{id}-old")),
        existed: false,
    })
}

/// Removes a file or a whole folder, never following a symbolic link.
pub(crate) fn remove(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The only paths a restore may replace.
fn restorable(location: &rbl_db::LibraryLocation) -> Vec<PathBuf> {
    let mut paths = vec![
        location.master_db.clone(),
        sidecar(&location.master_db, "-wal"),
        sidecar(&location.master_db, "-shm"),
        crate::analysis_dir(location),
        crate::artwork_dir(location),
    ];
    if let Some(root) = location.master_db.parent() {
        paths.extend(LIBRARY_FILES.iter().map(|name| root.join(name)));
    }
    paths
}

fn staged_name(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(PREFIX))
}

/// Rolls back an uncommitted restore, or finishes cleaning up a committed
/// one. Safe to repeat if it is itself interrupted.
pub fn recover(root: &Path, location: &rbl_db::LibraryLocation) -> Result<()> {
    let bytes = match fs::read(path(root)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let journal: Journal = serde_json::from_slice(&bytes)?;
    if journal.library != location.master_db {
        return Err(refused("An interrupted restore belongs to another library."));
    }
    crate::writable(location)?;
    let allowed = restorable(location);
    for swap in &journal.swaps {
        if !allowed.contains(&swap.target)
            || swap.staged.parent() != swap.target.parent()
            || swap.previous.parent() != swap.target.parent()
            || !staged_name(&swap.staged)
            || !staged_name(&swap.previous)
        {
            return Err(refused("Invalid restore recovery paths."));
        }
    }
    for swap in journal.swaps.iter().rev() {
        if !journal.committed {
            if swap.previous.exists() {
                remove(&swap.target)?;
                fs::rename(&swap.previous, &swap.target)?;
            } else if !swap.existed && !swap.staged.exists() {
                remove(&swap.target)?;
            }
        }
        remove(&swap.staged)?;
        remove(&swap.previous)?;
        rbl_core::durable::sync_dir(swap.target.parent().ok_or_else(|| refused("Invalid restore path"))?)?;
    }
    remove(&path(root))?;
    Ok(rbl_core::durable::sync_dir(root)?)
}

/// Removes staged copies left beside the library by a restore that stopped
/// before its journal was saved, when nothing had been replaced yet. Does
/// nothing while a journal is pending: its files are recovery's to handle.
pub fn remove_orphans(root: &Path, location: &rbl_db::LibraryLocation) -> Result<()> {
    if pending(root) {
        return Ok(());
    }
    let mut parents: Vec<PathBuf> = restorable(location).iter().filter_map(|path| path.parent().map(Path::to_path_buf)).collect();
    parents.sort();
    parents.dedup();
    for parent in parents {
        let Ok(entries) = fs::read_dir(&parent) else { continue };
        for entry in entries {
            let path = entry?.path();
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with(PREFIX) && (name.ends_with("-new") || name.ends_with("-old")) {
                tracing::info!(path = %path.display(), "removing an interrupted restore's staged copy");
                remove(&path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_restore_rolls_back_and_committed_restore_only_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let location = crate::testing::library(dir.path());
        let root = dir.path().join("state");
        let target = crate::analysis_dir(&location);
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("grid"), b"before").unwrap();
        let parent = target.parent().unwrap();
        let staged = parent.join(".rbxport-restore-test-new");
        let previous = parent.join(".rbxport-restore-test-old");
        for committed in [false, true] {
            fs::create_dir(&staged).unwrap();
            fs::write(staged.join("grid"), b"after").unwrap();
            let journal = Journal {
                library: location.master_db.clone(),
                committed,
                swaps: vec![Swap { target: target.clone(), staged: staged.clone(), previous: previous.clone(), existed: true }],
            };
            save(&root, &journal).unwrap();
            assert!(pending(&root));
            fs::rename(&target, &previous).unwrap();
            fs::rename(&staged, &target).unwrap();
            recover(&root, &location).unwrap();
            recover(&root, &location).unwrap();
            assert_eq!(fs::read(target.join("grid")).unwrap(), if committed { b"after".as_slice() } else { b"before".as_slice() });
            assert!(!previous.exists());
            assert!(!pending(&root));
        }
    }

    #[test]
    fn journals_naming_other_paths_or_libraries_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let location = crate::testing::library(dir.path());
        let root = dir.path().join("state");
        let outside = dir.path().join("elsewhere.txt");
        fs::write(&outside, b"keep").unwrap();
        let swap = Swap { target: outside.clone(), staged: dir.path().join(".rbxport-restore-x-new"), previous: dir.path().join(".rbxport-restore-x-old"), existed: true };
        save(&root, &Journal { library: location.master_db.clone(), committed: false, swaps: vec![swap] }).unwrap();
        assert!(recover(&root, &location).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"keep");
        save(&root, &Journal { library: dir.path().join("other.db"), committed: false, swaps: Vec::new() }).unwrap();
        assert!(recover(&root, &location).is_err());
    }

    #[test]
    fn orphaned_staged_copies_are_removed_only_without_a_journal() {
        let dir = tempfile::tempdir().unwrap();
        let location = crate::testing::library(dir.path());
        let root = dir.path().join("state");
        let parent = crate::analysis_dir(&location).parent().unwrap().to_path_buf();
        fs::create_dir_all(parent.join(".rbxport-restore-abc-new/inner")).unwrap();
        let db_orphan = location.master_db.parent().unwrap().join(".rbxport-restore-def-old");
        fs::write(&db_orphan, b"old").unwrap();
        let unrelated = parent.join(".rbxport-before-restore-keep");
        fs::create_dir_all(&unrelated).unwrap();
        save(&root, &Journal { library: location.master_db.clone(), committed: true, swaps: Vec::new() }).unwrap();
        remove_orphans(&root, &location).unwrap();
        assert!(db_orphan.exists());
        recover(&root, &location).unwrap();
        remove_orphans(&root, &location).unwrap();
        assert!(!db_orphan.exists());
        assert!(!parent.join(".rbxport-restore-abc-new").exists());
        assert!(unrelated.exists());
    }
}
