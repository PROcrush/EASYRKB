//! Only temporary USB diagnosis snapshots and their reports belong to this cache.
use std::fs;
use std::io;
use std::path::Path;

fn owned(name: &str) -> bool {
    name.starts_with("usb-recovery-")
        || name.strip_prefix("EASYRKB-")
            .and_then(|value| value.strip_suffix(".json"))
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok())
}

fn root_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(io::Error::other("The diagnosis cache path is not a regular local folder.")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn bytes(path: &Path) -> io::Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut total = 0_u64;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            total = total.saturating_add(bytes(&entry?.path())?);
        }
    }
    Ok(total)
}

pub(crate) fn usage(path: &Path) -> io::Result<u64> {
    if !root_exists(path)? {
        return Ok(0);
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if owned(&entry.file_name().to_string_lossy()) {
            total = total.saturating_add(bytes(&entry.path())?);
        }
    }
    Ok(total)
}

pub(crate) fn clear(path: &Path) -> io::Result<()> {
    if !root_exists(path)? {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !owned(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn clearing_owns_only_diagnosis_files_and_preserves_backups() {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path().join("cache");
        let snapshot = cache.join("usb-recovery-test/assets");
        fs::create_dir_all(&snapshot).unwrap();
        fs::write(snapshot.join("analysis.DAT"), [1_u8; 32]).unwrap();
        let report = cache.join(format!("EASYRKB-{}.json", uuid::Uuid::new_v4()));
        fs::write(&report, [2_u8; 16]).unwrap();
        fs::write(cache.join("unrelated.json"), [3_u8; 8]).unwrap();
        let backups = directory.path().join("backups/EASYRKB-before-repair-test");
        fs::create_dir_all(&backups).unwrap();
        fs::write(backups.join("export.pdb"), [4_u8; 24]).unwrap();
        assert_eq!(usage(&cache).unwrap(), 48);
        clear(&cache).unwrap();
        assert_eq!(usage(&cache).unwrap(), 0);
        assert!(!report.exists());
        assert_eq!(fs::read(cache.join("unrelated.json")).unwrap(), [3_u8; 8]);
        assert_eq!(fs::read(backups.join("export.pdb")).unwrap(), [4_u8; 24]);
    }

    #[cfg(unix)]
    #[test]
    fn clearing_never_follows_a_link_to_an_external_folder() {
        let cache = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("export.pdb"), b"keep").unwrap();
        std::os::unix::fs::symlink(external.path(), cache.path().join("usb-recovery-link")).unwrap();
        assert_eq!(usage(cache.path()).unwrap(), 0);
        clear(cache.path()).unwrap();
        assert_eq!(fs::read(external.path().join("export.pdb")).unwrap(), b"keep");
        let root_link = cache.path().join("root-link");
        std::os::unix::fs::symlink(external.path(), &root_link).unwrap();
        assert!(clear(&root_link).is_err());
    }
}
