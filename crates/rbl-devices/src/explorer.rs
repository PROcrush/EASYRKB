//! The folders the tree's Explorer section starts from, and what is under one.
//!
//! Nothing here walks a disk. [`roots`] asks the OS what is mounted, and
//! [`subfolders`] and [`audio_files`] read exactly one directory each — the
//! tree asks for a folder's children when it is opened and not before, which
//! is what keeps a home folder with a million files under it from costing
//! anything until somebody goes looking.
//!
//! What rekordbox lists [OBS], from the capture of its Explorer at `/`:
//! `.nofollow`, `Applications`, `Library`, `System`, `Users` — and not
//! `Volumes`, `bin`, `usr`, `private` or the `etc` / `tmp` / `var` symlinks,
//! every one of which carries the Finder's hidden flag. `.resolve` and
//! `.vol`, identical to `.nofollow` in every attribute readable from here,
//! were not listed either; why one dot-folder appeared and two did not is
//! [UNKNOWN]. This lists what the Finder would: no dot-names, nothing flagged
//! hidden, no symlinks.

use std::path::{Path, PathBuf};

/// One folder, or one file, in the Explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What to show: the last path component, or a volume's name for a root.
    pub name: String,
    pub path: PathBuf,
}

/// What one directory read produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// The first `cap` by name.
    pub entries: Vec<Entry>,
    /// How many there were, so a folder cut at the cap can say how many
    /// more it holds rather than pretending to be smaller.
    pub total: usize,
}

impl Listing {
    /// Whether the folder held more than were kept.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.total > self.entries.len()
    }
}

/// Extensions rekordbox plays. Mirrors `rbl_db::import::AUDIO_EXTENSIONS`,
/// which this crate does not otherwise need; a test in `src-tauri`, which
/// depends on both, holds the two equal.
pub const AUDIO_EXTENSIONS: &[&str] =
    &["mp3", "m4a", "aac", "flac", "wav", "aiff", "aif", "ogg", "opus"];

/// Where the Explorer starts, in the order rekordbox lists them [OBS]: the
/// music folder, the home folder, the system volume, then every other
/// mounted volume.
///
/// On macOS the system volume is named by the OS — `Macintosh HD` — and the
/// others are what is under `/Volumes`, minus the symlink back to `/` and the
/// hidden `.timemachine` mount. On Windows the system drive is `C:` and the
/// others are the remaining letters. How rekordbox names a Windows drive is
/// not captured; the volume label with the letter after it, as the Windows
/// Explorer shows it, is [ASSUME].
#[must_use]
pub fn roots() -> Vec<Entry> {
    let mut out = Vec::with_capacity(4);
    if let Some(music) = dirs::audio_dir().filter(|p| p.is_dir()) {
        out.push(Entry { name: last_component(&music), path: music });
    }
    if let Some(home) = dirs::home_dir().filter(|p| p.is_dir()) {
        out.push(Entry { name: last_component(&home), path: home });
    }
    // One refresh for both the system volume's name and the other volumes:
    // each refresh is a `statfs` of every mount, which waits on a sleeping
    // card reader.
    let disks = sysinfo::Disks::new_with_refreshed_list();
    if let Some(system) = system_volume(&disks) {
        out.push(system);
    }
    let volumes = if std::env::var_os(crate::FAKE_VOLUMES).is_some() {
        crate::list()
    } else {
        crate::devices_from(&disks)
    };
    for device in volumes {
        if is_hidden_name(&device.name) {
            continue;
        }
        out.push(Entry { name: device.name, path: device.mount_point });
    }
    // The same volume can be reached two ways — a fake volume set to the
    // home folder, say — and one row is enough.
    let mut seen = std::collections::HashSet::new();
    out.retain(|entry| seen.insert(entry.path.clone()));
    out
}

/// The volume the OS boots from, named the way the OS names it.
fn system_volume(disks: &sysinfo::Disks) -> Option<Entry> {
    let root: PathBuf = if cfg!(windows) {
        std::env::var_os("SystemDrive").map_or_else(
            || PathBuf::from("C:\\"),
            |d| PathBuf::from(format!("{}\\", d.to_string_lossy())),
        )
    } else {
        PathBuf::from("/")
    };
    if !root.is_dir() {
        return None;
    }
    let label = disks
        .list()
        .iter()
        .find(|disk| disk.mount_point() == root)
        .map(|disk| disk.name().to_string_lossy().into_owned())
        .filter(|name| !name.is_empty());
    let text = root.to_string_lossy();
    let name = match label {
        Some(label) if cfg!(windows) => format!("{label} ({})", text.trim_end_matches('\\')),
        Some(label) => label,
        None => text.into_owned(),
    };
    Some(Entry { name, path: root })
}

fn last_component(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

/// The folders directly under `path`, by name, the first `cap` of them.
///
/// The whole directory is read and sorted before the cap is applied, so
/// what is kept is the first `cap` by name and not the first `cap` the disk
/// happened to hand back. A folder of 14,503 subfolders — the reference
/// library's card has one [OBS] — reads in 24 ms warm.
///
/// A folder that cannot be read — permission denied, gone, not a folder —
/// lists as empty rather than as an error: the tree shows an empty branch,
/// which is what a folder you may not look inside is.
#[must_use]
pub fn subfolders(path: &Path, cap: usize) -> Listing {
    read(path, cap, |file_type, _| file_type.is_dir())
}

/// The audio files directly under `path`, by name, the first `cap` of them.
///
/// One level only. Rekordbox's Explorer shows a folder's own files and the
/// tree shows its subfolders; recursing here would turn a click on a volume
/// into a walk of the disk.
#[must_use]
pub fn audio_files(path: &Path, cap: usize) -> Listing {
    read(path, cap, |file_type, name| file_type.is_file() && is_audio(Path::new(name)))
}

fn read(path: &Path, cap: usize, wanted: impl Fn(&std::fs::FileType, &str) -> bool) -> Listing {
    let Ok(dir) = std::fs::read_dir(path) else { return Listing::default() };
    let mut entries: Vec<Entry> = Vec::new();
    for entry in dir.filter_map(Result::ok) {
        // From the directory read itself where the platform gives it there,
        // so this is not a stat per entry on the platforms that matter.
        let Ok(file_type) = entry.file_type() else { continue };
        // A symlink is skipped whatever it points at: `/Volumes/Macintosh HD`
        // points back at `/`, and a tree that follows it never ends.
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !wanted(&file_type, &name) || is_hidden_name(&name) {
            continue;
        }
        // The flag needs a stat, so it is read for folders only: a hidden
        // audio file is a dot-name in practice, and a folder of three
        // thousand tracks on a slow card should not pay three thousand stats.
        if file_type.is_dir() && is_flagged_hidden(&entry) {
            continue;
        }
        entries.push(Entry { name, path: entry.path() });
    }
    // The order the Finder and the capture use: by name, case aside. A
    // directory read comes back in disk order, which is no order at all.
    entries.sort_by(|a, b| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name))
    });
    let total = entries.len();
    entries.truncate(cap);
    Listing { entries, total }
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

/// The Finder's hidden flag (`UF_HIDDEN`) on macOS, `FILE_ATTRIBUTE_HIDDEN`
/// on Windows. Neither is a dot-name: `/bin` and `/usr` are hidden by flag
/// alone, and the capture shows rekordbox leaving them out.
#[cfg(target_os = "macos")]
fn is_flagged_hidden(entry: &std::fs::DirEntry) -> bool {
    use std::os::macos::fs::MetadataExt;
    const UF_HIDDEN: u32 = 0x8000;
    entry.metadata().is_ok_and(|m| m.st_flags() & UF_HIDDEN != 0)
}

#[cfg(windows)]
fn is_flagged_hidden(entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    entry.metadata().is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn is_flagged_hidden(_entry: &std::fs::DirEntry) -> bool {
    false
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.as_str()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn names(listing: &Listing) -> Vec<&str> {
        listing.entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn subfolders_are_the_folders_by_name_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["zeta", "Alpha", "beta"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("track.mp3"), b"").unwrap();
        std::fs::create_dir(dir.path().join(".hidden")).unwrap();
        let listing = subfolders(dir.path(), 100);
        assert_eq!(names(&listing), ["Alpha", "beta", "zeta"]);
        assert!(!listing.truncated());
        assert_eq!(listing.total, 3);
        assert_eq!(listing.entries[0].path, dir.path().join("Alpha"));
    }

    #[test]
    fn audio_files_are_the_playable_files_by_name() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.mp3", "A.FLAC", "notes.txt", "cover.jpg", ".DS_Store", "c.aiff"] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub.mp3")).unwrap();
        assert_eq!(names(&audio_files(dir.path(), 100)), ["A.FLAC", "b.mp3", "c.aiff"]);
    }

    #[test]
    fn a_folder_past_the_cap_keeps_the_first_by_name_and_says_how_many_there_were() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["j", "b", "h", "a", "f", "d", "c", "g", "e", "i"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        let listing = subfolders(dir.path(), 4);
        assert_eq!(names(&listing), ["a", "b", "c", "d"]);
        assert_eq!(listing.total, 10);
        assert!(listing.truncated());
    }

    #[test]
    fn a_folder_that_cannot_be_read_lists_as_empty() {
        assert_eq!(subfolders(Path::new("/no/such/folder"), 10), Listing::default());
        assert_eq!(audio_files(Path::new(""), 10), Listing::default());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.mp3");
        std::fs::write(&file, b"").unwrap();
        // A file is not a folder, and asking is not an error.
        assert_eq!(subfolders(&file, 10), Listing::default());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_left_out_even_when_it_points_at_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("real")).unwrap();
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();
        assert_eq!(names(&subfolders(dir.path(), 10)), ["real"]);
    }

    #[test]
    fn the_roots_start_with_the_music_and_home_folders() {
        let roots = roots();
        // Every machine this runs on has a home folder; the music folder is
        // usually there too but a bare CI runner may lack it.
        let home = dirs::home_dir().unwrap();
        assert!(roots.iter().any(|r| r.path == home), "{roots:?}");
        let mut paths: Vec<&PathBuf> = roots.iter().map(|r| &r.path).collect();
        let before = paths.len();
        paths.dedup();
        assert_eq!(paths.len(), before, "a volume listed twice: {roots:?}");
        for root in &roots {
            assert!(!root.name.is_empty(), "{root:?}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_system_volume_is_the_root_named_as_the_os_names_it() {
        let system = system_volume(&sysinfo::Disks::new_with_refreshed_list()).unwrap();
        assert_eq!(system.path, Path::new("/"));
        // `Macintosh HD` on every Mac that kept the default; not `/`.
        assert_ne!(system.name, "/");
    }
}
