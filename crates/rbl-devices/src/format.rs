//! Erases a whole USB stick, every partition on it, and lays it out again
//! with a file system a player can read.
//!
//! Only macOS has an implementation (`diskutil`). Everywhere else the call
//! reports that formatting is not supported, so nothing is ever erased by a
//! code path that has not been exercised.
use std::{io, path::Path};

/// How the stick is laid out afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One FAT32 partition: readable on macOS, Windows, Linux and CDJs.
    Fat32,
    /// One Mac OS Extended (Journaled) partition: readable on macOS only.
    HfsPlus,
}

/// Erases the stick mounted at `path` and formats it as `layout`, keeping the
/// name the volume has now.
///
/// Every partition on the physical disk is destroyed, not only the one that
/// is mounted at `path`. A path that is not a mounted USB volume is refused,
/// as it is for [`crate::eject::eject`].
///
/// # Errors
///
/// Fails if `path` is not a connected external volume, if the volume is in use
/// (nothing is forced), or if the platform has no formatter.
pub fn format(path: &Path, layout: Layout) -> io::Result<()> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let devices = super::devices_from(&disks);
    let Some(device) = devices.iter().find(|device| device.mount_point == path) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "The USB volume is no longer connected.",
        ));
    };
    platform_format(path, layout, &device.name)
}

/// The label a FAT volume can carry: at most 11 characters, upper case, and
/// none of the characters FAT forbids. Falls back to `USB` when nothing is
/// left.
#[must_use]
pub fn fat_label(name: &str) -> String {
    let label: String = name
        .chars()
        .filter(|c| !c.is_control() && !"\"*+,./:;<=>?[\\]|".contains(*c))
        .flat_map(char::to_uppercase)
        .take(11)
        .collect();
    let label = label.trim().to_owned();
    if label.is_empty() { "USB".to_owned() } else { label }
}

/// The `diskutil` arguments that erase `disk` (such as `disk5`).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn diskutil_arguments(disk: &str, layout: Layout, name: &str) -> Vec<String> {
    let device = format!("/dev/{disk}");
    let fat = fat_label(name);
    // A Mac volume name may not contain a colon, and `diskutil` reads an
    // empty one as a missing argument.
    let hfs = name.replace(':', "-");
    let hfs = if hfs.trim().is_empty() { "USB".to_owned() } else { hfs };
    match layout {
        Layout::Fat32 => ["eraseDisk", "FAT32", &fat, "MBRFormat", &device]
            .map(str::to_owned)
            .to_vec(),
        Layout::HfsPlus => ["eraseDisk", "JHFS+", &hfs, "GPT", &device]
            .map(str::to_owned)
            .to_vec(),
    }
}

/// The whole disk a `diskutil info` report says its volume is part of, when
/// that disk is an external one.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn external_whole_disk(info: &str) -> Result<String, &'static str> {
    let field = |key: &str| {
        info.lines()
            .find_map(|line| line.trim().strip_prefix(key))
            .map(str::trim)
    };
    if field("Device Location:") != Some("External") {
        return Err("Only an external USB drive can be formatted.");
    }
    match field("Part of Whole:") {
        Some(disk) if disk.starts_with("disk") && disk[4..].chars().all(|c| c.is_ascii_digit()) => {
            Ok(disk.to_owned())
        }
        _ => Err("Could not identify the USB drive."),
    }
}

#[cfg(target_os = "macos")]
fn platform_format(path: &Path, layout: Layout, name: &str) -> io::Result<()> {
    let info = std::process::Command::new("/usr/sbin/diskutil")
        .arg("info")
        .arg(path)
        .output()?;
    if !info.status.success() {
        return Err(io::Error::other("Could not identify the USB drive."));
    }
    let disk = external_whole_disk(&String::from_utf8_lossy(&info.stdout))
        .map_err(io::Error::other)?;
    let output = std::process::Command::new("/usr/sbin/diskutil")
        .args(diskutil_arguments(&disk, layout, name))
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    let detail = detail.trim();
    Err(io::Error::other(if detail.is_empty() {
        "The system could not format the device. It may be in use.".to_owned()
    } else {
        detail.to_owned()
    }))
}

#[cfg(not(target_os = "macos"))]
fn platform_format(_: &Path, _: Layout, _: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Formatting is only supported on macOS.",
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_is_never_treated_as_a_volume_to_format() {
        let directory = tempfile::tempdir().unwrap();
        let error = format(directory.path(), Layout::Fat32).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(directory.path().is_dir());
    }

    #[test]
    fn a_fat_label_is_upper_case_short_and_legal() {
        assert_eq!(fat_label("DJ Stick"), "DJ STICK");
        assert_eq!(fat_label("A very long stick name"), "A VERY LONG");
        assert_eq!(fat_label("a:b/c"), "ABC");
        assert_eq!(fat_label("***"), "USB");
        assert_eq!(fat_label(""), "USB");
    }

    #[test]
    fn one_fat32_partition_is_a_single_erase_of_the_whole_disk() {
        assert_eq!(
            diskutil_arguments("disk5", Layout::Fat32, "USB B"),
            ["eraseDisk", "FAT32", "USB B", "MBRFormat", "/dev/disk5"],
        );
    }

    #[test]
    fn hfs_plus_takes_the_whole_disk_under_the_same_name() {
        assert_eq!(
            diskutil_arguments("disk5", Layout::HfsPlus, "DJ: Stick"),
            ["eraseDisk", "JHFS+", "DJ- Stick", "GPT", "/dev/disk5"],
        );
    }

    #[test]
    fn only_an_external_disk_is_erased() {
        let external = "   Part of Whole:             disk5\n   Device Location:           External\n";
        assert_eq!(external_whole_disk(external), Ok("disk5".to_owned()));
        let internal = "   Part of Whole:             disk3\n   Device Location:           Internal\n";
        assert!(external_whole_disk(internal).is_err());
        assert!(external_whole_disk("").is_err());
        let odd = "   Part of Whole:             ../x\n   Device Location:           External\n";
        assert!(external_whole_disk(odd).is_err());
    }
}
