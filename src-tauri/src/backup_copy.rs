//! File copies for snapshots. Native APIs can clone data blocks; the caller
//! owns the staging directory and removes it if copying or cancellation fails.
use std::{fs, io, path::Path};

type Progress<'a> = dyn FnMut(u64) -> io::Result<()> + 'a;
type TreeProgress<'a> = dyn FnMut(u64, Option<&Path>) -> io::Result<()> + 'a;

pub fn copy_file(source: &Path, target: &Path, progress: &mut Progress<'_>) -> io::Result<u64> {
    progress(0)?;
    let meta = fs::symlink_metadata(source)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a regular backup file",
        ));
    }
    #[cfg(target_os = "macos")]
    if mac::try_clone(source, target)? {
        fs::File::open(target)?.sync_all()?;
        progress(meta.len())?;
        return Ok(meta.len());
    }
    #[cfg(windows)]
    {
        windows::copy(source, target, meta.len(), progress)?;
        return Ok(meta.len());
    }
    #[cfg(not(windows))]
    buffered_copy(source, target, progress)
}

/// Copy independent analysis files with bounded I/O concurrency. The caller's
/// callback stays on this thread; all workers finish before an error returns,
/// so the caller can safely remove the incomplete snapshot.
#[cfg(test)]
fn copy_tree_with_workers(
    source: &Path,
    target: &Path,
    progress: &mut Progress<'_>,
    workers: usize,
) -> io::Result<u64> {
    TreeCopyPlan::prepare(source, target, progress)?.copy_with_workers(
        &mut |bytes, _| progress(bytes),
        workers,
        false,
    )
}

pub struct TreeCopyPlan {
    directories: Vec<std::path::PathBuf>,
    files: Vec<(std::path::PathBuf, std::path::PathBuf)>,
    pub bytes: u64,
}

impl TreeCopyPlan {
    /// Enumerate and size once; copying consumes this exact list.
    pub fn prepare(source: &Path, target: &Path, progress: &mut Progress<'_>) -> io::Result<Self> {
        let mut bytes = 0;
        let mut pending = vec![(source.to_path_buf(), target.to_path_buf())];
        let mut directories = Vec::new();
        let mut files = Vec::new();
        while let Some((source, target)) = pending.pop() {
            progress(0)?;
            let meta = fs::symlink_metadata(&source)?;
            if meta.file_type().is_symlink() || (!meta.is_dir() && !meta.is_file()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Unsupported file in backup",
                ));
            }
            if meta.is_dir() {
                for entry in fs::read_dir(&source)? {
                    let entry = entry?;
                    pending.push((entry.path(), target.join(entry.file_name())));
                }
                directories.push(target);
            } else {
                bytes += meta.len();
                files.push((source, target));
            }
        }
        Ok(Self {
            directories,
            files,
            bytes,
        })
    }

    #[cfg(test)]
    pub fn copy(self, progress: &mut TreeProgress<'_>) -> io::Result<u64> {
        let workers = std::thread::available_parallelism().map_or(2, usize::from);
        self.copy_with_workers(progress, workers, false)
    }

    pub fn compress(self, progress: &mut TreeProgress<'_>) -> io::Result<u64> {
        let workers = std::thread::available_parallelism().map_or(2, usize::from);
        self.copy_with_workers(progress, workers, true)
    }

    fn copy_with_workers(
        self,
        progress: &mut TreeProgress<'_>,
        workers: usize,
        compressed: bool,
    ) -> io::Result<u64> {
        use std::sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            mpsc,
        };
        let Self {
            directories, files, ..
        } = self;
        for directory in &directories {
            progress(0, None)?;
            fs::create_dir_all(directory)?;
        }
        let next = AtomicUsize::new(0);
        let stopped = AtomicBool::new(false);
        let mut bytes = 0;
        let mut failure = None;
        std::thread::scope(|scope| {
            // Bound queued updates as well as workers, even for tiny cloned files.
            let (sender, receiver) = mpsc::sync_channel::<io::Result<(usize, u64)>>(32);
            for _ in 0..workers.max(1).min(files.len()) {
                let sender = sender.clone();
                let files = &files;
                let next = &next;
                let stopped = &stopped;
                scope.spawn(move || {
                    while !stopped.load(Ordering::Acquire) {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((source, target)) = files.get(index) else {
                            break;
                        };
                        let operation = if compressed {
                            crate::backup_zip::compress_file
                        } else {
                            copy_file
                        };
                        let result = operation(source, target, &mut |bytes| {
                            if stopped.load(Ordering::Acquire) {
                                return Err(io::Error::new(
                                    io::ErrorKind::Interrupted,
                                    "Backup stopped",
                                ));
                            }
                            sender
                                .send(Ok((index, bytes)))
                                .map_err(|_| io::Error::other("Backup progress disconnected"))
                        });
                        if let Err(error) = result {
                            let _ = sender.send(Err(error));
                            stopped.store(true, Ordering::Release);
                            break;
                        }
                    }
                });
            }
            drop(sender);
            for update in receiver {
                if failure.is_some() {
                    continue;
                }
                let result = update.and_then(|(index, delta)| {
                    bytes += delta;
                    progress(delta, files.get(index).map(|(source, _)| source.as_path()))
                });
                if let Err(error) = result {
                    failure = Some(error);
                    stopped.store(true, Ordering::Release);
                }
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        // Children must be durable before their parent. Never publish a snapshot
        // until every worker and every directory flush has finished.
        for directory in directories.iter().rev() {
            progress(0, None)?;
            crate::durable::sync_dir(directory)?;
        }
        Ok(bytes)
    }
}

#[cfg(any(not(windows), test))]
fn buffered_copy(source: &Path, target: &Path, progress: &mut Progress<'_>) -> io::Result<u64> {
    use io::{Read, Write};
    let mut input = fs::File::open(source)?;
    let meta = input.metadata()?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let mut buffer = vec![0; 1024 * 1024];
    let mut bytes = 0;
    loop {
        progress(0)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        bytes += count as u64;
        progress(count as u64)?;
    }
    output.set_permissions(meta.permissions())?;
    output.sync_all()?;
    Ok(bytes)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod mac {
    use super::{io, Path};
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    pub(super) fn try_clone(source: &Path, target: &Path) -> io::Result<bool> {
        let source = CString::new(source.as_os_str().as_bytes())?;
        let target = CString::new(target.as_os_str().as_bytes())?;
        // CLONE_NOFOLLOW | CLONE_NOOWNERCOPY; source and destination are
        // distinct, NUL-terminated paths alive for this synchronous call.
        let result = unsafe { libc::clonefile(source.as_ptr(), target.as_ptr(), 0x0001 | 0x0002) };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EXDEV | libc::ENOTSUP | libc::ENOSYS) => Ok(false),
            _ => Err(error),
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use super::*;
    use std::{
        ffi::c_void,
        os::windows::ffi::OsStrExt,
        panic::{catch_unwind, AssertUnwindSafe},
    };
    use windows_sys::Win32::Storage::FileSystem::*;

    struct Context<'a> {
        progress: &'a mut Progress<'a>,
        reported: u64,
        length: u64,
        error: Option<io::Error>,
    }

    unsafe extern "system" fn notify(
        message: *const COPYFILE2_MESSAGE,
        context: *const c_void,
    ) -> COPYFILE2_MESSAGE_ACTION {
        // CopyFile2 calls back synchronously. The context is exclusively
        // borrowed for the call; the message and its tagged union are OS-owned.
        let context = unsafe { &mut *context.cast_mut().cast::<Context<'_>>() };
        let message = unsafe { &*message };
        if context.error.is_some() {
            return COPYFILE2_PROGRESS_CANCEL;
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let copied = if message.Type == COPYFILE2_CALLBACK_CHUNK_FINISHED {
                unsafe { message.Info.ChunkFinished.uliTotalBytesTransferred }.min(context.length)
            } else {
                context.reported
            };
            let delta = copied.saturating_sub(context.reported);
            (context.progress)(delta)?;
            context.reported += delta;
            Ok(())
        }))
        .unwrap_or_else(|_| Err(io::Error::other("Backup progress callback failed")));
        match result {
            Ok(()) => COPYFILE2_PROGRESS_CONTINUE,
            Err(error) => {
                context.error = Some(error);
                COPYFILE2_PROGRESS_CANCEL
            }
        }
    }

    fn wide(path: &Path) -> io::Result<Vec<u16>> {
        // Canonicalize the existing parent too, so new targets use extended
        // paths and work beyond MAX_PATH, including on UNC shares.
        let absolute = if path.exists() {
            path.canonicalize()?
        } else {
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("Missing copy destination parent"))?;
            parent.canonicalize()?.join(
                path.file_name()
                    .ok_or_else(|| io::Error::other("Missing copy filename"))?,
            )
        };
        let mut value: Vec<u16> = absolute.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NUL in backup path",
            ));
        }
        value.push(0);
        Ok(value)
    }

    pub(super) fn copy(
        source: &Path,
        target: &Path,
        length: u64,
        progress: &mut Progress<'_>,
    ) -> io::Result<()> {
        let source = wide(source)?;
        let destination = wide(target)?;
        let mut context = Context {
            progress,
            reported: 0,
            length,
            error: None,
        };
        let parameters = COPYFILE2_EXTENDED_PARAMETERS {
            dwSize: std::mem::size_of::<COPYFILE2_EXTENDED_PARAMETERS>() as u32,
            dwCopyFlags: COPY_FILE_FAIL_IF_EXISTS,
            pfCancel: std::ptr::null_mut(),
            pProgressRoutine: Some(notify),
            pvCallbackContext: (&mut context as *mut Context<'_>).cast(),
        };
        // All pointer targets outlive CopyFile2. Supported ReFS volumes can
        // clone automatically; NTFS and other volumes use native copying.
        let result = unsafe { CopyFile2(source.as_ptr(), destination.as_ptr(), &parameters) };
        if let Some(error) = context.error {
            return Err(error);
        }
        if result < 0 {
            return Err(io::Error::other(format!(
                "Windows copy failed (HRESULT {result:#010x})"
            )));
        }
        // A cloned or empty file need not generate chunk callbacks.
        (context.progress)(length.saturating_sub(context.reported))?;
        let permissions = fs::metadata(target)?.permissions();
        // FlushFileBuffers needs a writable handle. Restore the source's
        // read-only attribute after flushing the newly created backup file.
        if permissions.readonly() {
            let mut writable = permissions.clone();
            writable.set_readonly(false);
            fs::set_permissions(target, writable)?;
        }
        let synced = fs::OpenOptions::new()
            .write(true)
            .open(target)
            .and_then(|file| file.sync_all());
        if permissions.readonly() {
            fs::set_permissions(target, permissions)?;
        }
        synced
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn tree_fixture(root: &Path, count: usize) -> std::path::PathBuf {
        let source = root.join("source");
        fs::create_dir_all(source.join("empty/nested")).unwrap();
        for i in 0..count {
            let directory = source.join(format!("{i:04}"));
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                directory.join("analysis.dat"),
                vec![u8::try_from(i % 256).unwrap_or(0); 64 * 1024],
            )
            .unwrap();
        }
        source
    }

    #[test]
    fn copy_reuses_the_sized_file_list_without_enumerating_again() {
        let dir = tempfile::tempdir().unwrap();
        let source = tree_fixture(dir.path(), 4);
        let target = dir.path().join("backup");
        let plan = TreeCopyPlan::prepare(&source, &target, &mut |_| Ok(())).unwrap();
        assert_eq!(plan.bytes, 4 * 64 * 1024);
        assert!(!target.exists(), "preparation only reads source metadata");
        // The app holds the edit gate during both phases. A new unrelated
        // entry here demonstrates that copying consumes the prepared list.
        fs::write(source.join("later.dat"), b"not in plan").unwrap();
        let mut reported_files = std::collections::HashSet::new();
        assert_eq!(
            plan.copy(&mut |_, path| {
                if let Some(path) = path {
                    reported_files.insert(path.to_path_buf());
                }
                Ok(())
            })
            .unwrap(),
            4 * 64 * 1024
        );
        assert_eq!(reported_files.len(), 4);
        assert!(reported_files.iter().all(|path| path.starts_with(&source)));
        assert!(!target.join("later.dat").exists());
        assert!(target.join("0000/analysis.dat").is_file());
    }

    #[test]
    fn parallel_tree_preserves_files_empty_directories_and_exact_progress() {
        let dir = tempfile::tempdir().unwrap();
        let source = tree_fixture(dir.path(), 24);
        let target = dir.path().join("backup");
        let mut reported = 0;
        let bytes = copy_tree_with_workers(
            &source,
            &target,
            &mut |delta| {
                reported += delta;
                Ok(())
            },
            4,
        )
        .unwrap();
        assert_eq!(bytes, 24 * 64 * 1024);
        assert_eq!(reported, bytes);
        assert!(target.join("empty/nested").is_dir());
        for i in 0..24 {
            let relative = format!("{i:04}/analysis.dat");
            assert_eq!(
                fs::read(source.join(&relative)).unwrap(),
                fs::read(target.join(&relative)).unwrap()
            );
        }
    }

    #[test]
    fn cancelled_parallel_copy_joins_workers_before_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let source = tree_fixture(dir.path(), 64);
        let target = dir.path().join("backup");
        let error = copy_tree_with_workers(
            &source,
            &target,
            &mut |bytes| {
                if bytes > 0 {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "User cancelled"))
                } else {
                    Ok(())
                }
            },
            4,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(error.to_string(), "User cancelled");
        fs::remove_dir_all(&target).unwrap();
        assert!(!target.exists());
        assert_eq!(
            fs::read(source.join("0063/analysis.dat")).unwrap(),
            vec![63; 64 * 1024]
        );
    }

    #[test]
    fn parallel_copy_keeps_existing_files_on_worker_failure() {
        let dir = tempfile::tempdir().unwrap();
        let source = tree_fixture(dir.path(), 8);
        let target = dir.path().join("backup");
        fs::create_dir_all(target.join("0003")).unwrap();
        fs::write(target.join("0003/analysis.dat"), b"keep").unwrap();
        assert!(copy_tree_with_workers(&source, &target, &mut |_| Ok(()), 4).is_err());
        assert_eq!(fs::read(target.join("0003/analysis.dat")).unwrap(), b"keep");
    }

    #[test]
    #[ignore = "manual copy throughput measurement"]
    #[allow(clippy::print_stdout, reason = "the point of this manual benchmark is its printed timing")]
    fn compare_serial_and_parallel_tree_copy() {
        let dir = tempfile::tempdir().unwrap();
        let source = tree_fixture(dir.path(), 512);
        for workers in [1, 4, 4, 1] {
            let target = dir.path().join("backup");
            let started = std::time::Instant::now();
            copy_tree_with_workers(&source, &target, &mut |_| Ok(()), workers).unwrap();
            println!("{workers} workers: {:?}", started.elapsed());
            fs::remove_dir_all(target).unwrap();
        }
    }

    #[test]
    fn native_copy_is_independent_and_reports_the_file_size() {
        use io::Write;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.dat");
        let target = dir.path().join("backup.dat");
        fs::write(&source, vec![7; 2 * 1024 * 1024]).unwrap();
        let mut reported = 0;
        let copied = copy_file(&source, &target, &mut |bytes| {
            reported += bytes;
            Ok(())
        })
        .unwrap();
        assert_eq!(copied, reported);
        assert_eq!(copied, 2 * 1024 * 1024);
        // In-place writes exercise copy-on-write, rather than replacing inodes.
        fs::OpenOptions::new()
            .write(true)
            .open(&source)
            .unwrap()
            .write_all(b"changed")
            .unwrap();
        assert_eq!(fs::read(&target).unwrap(), vec![7; 2 * 1024 * 1024]);
        fs::remove_file(source).unwrap();
        assert_eq!(fs::metadata(target).unwrap().len(), copied);
    }

    #[test]
    fn existing_destination_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("backup");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"saved").unwrap();
        assert!(copy_file(&source, &target, &mut |_| Ok(())).is_err());
        assert_eq!(fs::read(target).unwrap(), b"saved");
    }

    #[test]
    fn buffered_fallback_copies_and_stops_between_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("backup");
        fs::write(&source, vec![3; 3 * 1024 * 1024]).unwrap();
        let error = buffered_copy(&source, &target, &mut |bytes| {
            if bytes > 0 {
                Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert!(fs::metadata(&target).unwrap().len() < fs::metadata(&source).unwrap().len());
        fs::remove_file(&target).unwrap();
        buffered_copy(&source, &target, &mut |_| Ok(())).unwrap();
        assert_eq!(fs::read(source).unwrap(), fs::read(target).unwrap());
    }

    #[test]
    fn native_copy_honours_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("backup");
        fs::write(&source, vec![8; 2 * 1024 * 1024]).unwrap();
        let result = copy_file(&source, &target, &mut |bytes| {
            if bytes > 0 {
                Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert_eq!(fs::metadata(source).unwrap().len(), 2 * 1024 * 1024);
    }
}
