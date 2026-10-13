//! Safe OS ejection after all export handles have closed. Never force a busy volume.
use std::{io, path::Path};

pub fn eject(path: &Path) -> io::Result<()> {
    // Enumerate real volumes: test directories and arbitrary paths must never
    // turn into an eject request for the disk containing them.
    let disks = sysinfo::Disks::new_with_refreshed_list();
    if !super::devices_from(&disks)
        .iter()
        .any(|device| device.mount_point == path)
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "The USB volume is no longer connected.",
        ));
    }
    platform_eject(path)
}

#[cfg(not(windows))]
fn run(command: &mut std::process::Command) -> io::Result<std::process::Output> {
    let output = command.output()?;
    if output.status.success() {
        Ok(output)
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(io::Error::other(if detail.is_empty() {
            "The system could not eject the device. It may be in use.".to_owned()
        } else {
            detail
        }))
    }
}

#[cfg(target_os = "macos")]
fn platform_eject(path: &Path) -> io::Result<()> {
    run(std::process::Command::new("/usr/sbin/diskutil")
        .arg("eject")
        .arg(path))
    .map(|_| ())
}

#[cfg(target_os = "linux")]
fn platform_eject(path: &Path) -> io::Result<()> {
    let source = run(std::process::Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "--mountpoint"])
        .arg(path))?;
    let source = String::from_utf8_lossy(&source.stdout);
    let source = source.trim();
    if !source.starts_with("/dev/") || source.lines().count() != 1 {
        return Err(io::Error::other("Could not identify the USB volume."));
    }
    run(std::process::Command::new("udisksctl").args([
        "unmount",
        "--no-user-interaction",
        "--block-device",
        source,
    ]))
    .map(|_| ())
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "Windows volume operations require Win32 FFI; the owned handle closes on every path"
)]
fn platform_eject(path: &Path) -> io::Result<()> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
        System::{
            Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA},
            IO::DeviceIoControl,
        },
    };
    let text = path.as_os_str().to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() != 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a USB drive root.",
        ));
    }
    let volume: Vec<u16> = format!("\\\\.\\{}:", char::from(bytes[0]))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: NUL-terminated name and null optional pointers; synchronous handle.
    let raw = unsafe {
        CreateFileW(
            volume.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateFileW returned a new, valid handle owned by this function.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    for code in [
        FSCTL_LOCK_VOLUME,
        FSCTL_DISMOUNT_VOLUME,
        IOCTL_STORAGE_EJECT_MEDIA,
    ] {
        let mut returned = 0;
        // SAFETY: Live volume handle; these control codes use no input/output buffer.
        // Lock must succeed before dismount, so open files prevent ejection.
        let ok = unsafe {
            DeviceIoControl(
                handle.as_raw_handle(),
                code,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn platform_eject(_: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Eject is not supported on this platform.",
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    #[allow(clippy::unwrap_used)]
    fn a_directory_is_never_treated_as_a_volume_to_eject() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(super::eject(directory.path()).unwrap_err().kind(), std::io::ErrorKind::NotFound);
        assert!(directory.path().is_dir());
    }
}
