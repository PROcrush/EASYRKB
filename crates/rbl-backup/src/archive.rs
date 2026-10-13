//! Reading a backup ZIP: its small metadata entries, which part of the
//! library each of its other entries belongs to, and their contents.
use crate::{refused, Error, Result, LIBRARY_FILES};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
};
use zip::{CompressionMethod, ZipArchive};

pub type Archive = ZipArchive<fs::File>;

pub fn open(path: &Path) -> Result<Archive> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(refused("Choose an EASYRKB backup ZIP file."));
    }
    ZipArchive::new(fs::File::open(path)?).map_err(|_| refused("This file is not a readable ZIP archive."))
}

/// An entry of at most `limit` bytes, or `None` when the archive lacks it.
pub fn read_small(zip: &mut Archive, name: &str, limit: u64) -> Result<Option<Vec<u8>>> {
    let mut entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if entry.size() > limit {
        return Err(refused(format!("The backup's {name} is too large.")));
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Which part of the library an archive entry restores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Database,
    DatabaseWal,
    /// A path inside `analysis/`; empty for the folder itself.
    Analysis(PathBuf),
    /// A path inside `artwork/`; empty for the folder itself.
    Artwork(PathBuf),
    /// One of [`LIBRARY_FILES`].
    LibraryFile(&'static str),
    /// Metadata, or anything a restore does not use.
    Other,
}

/// Classifies an entry by its name. `None` for a name that could escape the
/// folder it is unpacked into; such an archive is not restored at all.
pub fn classify(name: &str) -> Option<Entry> {
    let path = Path::new(name);
    if path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return None;
    }
    let mut parts = path.components();
    let first = parts.next()?.as_os_str();
    let rest: PathBuf = parts.collect();
    Some(match first.to_str() {
        Some("master.db") if rest.as_os_str().is_empty() => Entry::Database,
        Some("master.db-wal") if rest.as_os_str().is_empty() => Entry::DatabaseWal,
        Some("analysis") => Entry::Analysis(rest),
        Some("artwork") => Entry::Artwork(rest),
        Some(file) if rest.as_os_str().is_empty() => LIBRARY_FILES
            .iter()
            .find(|known| **known == file)
            .map_or(Entry::Other, |known| Entry::LibraryFile(known)),
        _ => Entry::Other,
    })
}

/// A symbolic link stored in the archive. Never unpacked.
pub fn is_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170_000 == 0o120_000)
}

/// Entry names by index, without touching the entries themselves.
pub fn names(zip: &Archive) -> Vec<String> {
    zip.file_names().map(str::to_owned).collect()
}

/// What is known of an entry before its contents are read.
#[derive(Clone, Copy, Debug)]
pub struct Meta {
    /// Uncompressed bytes.
    pub size: u64,
    pub unix_mode: Option<u32>,
}

/// Reports bytes of an entry handled so far; an error stops the work.
pub type Report<'a> = dyn FnMut(u64) -> Result<()> + 'a;

/// Handles one entry: its position in the list asked for, what is known of
/// it, and a reader over its contents that fails at the end if they do not
/// match the archive's checksum.
pub type Each<'a> = dyn Fn(usize, Meta, &mut dyn Read, &mut Report<'_>) -> Result<()> + Sync + 'a;

/// Entries up to this compressed size go to a worker in memory; larger ones
/// are unpacked by the reading thread as it reaches them.
const IN_MEMORY: u64 = 8 * 1024 * 1024;

struct Job {
    position: usize,
    meta: Meta,
    method: CompressionMethod,
    crc: u32,
    data: Vec<u8>,
}

enum Update {
    Bytes(usize, u64),
    Failed(Error),
}

/// Runs `each` on the entries at `indexes`. `progress` gets each entry's
/// position and the bytes `each` reports, on the calling thread.
///
/// The calling thread alone reads the archive, in order, and the other cores
/// inflate and handle what it read. Several threads reading one large file
/// at once spend most of their time contending in the kernel: on macOS, 14
/// threads measuring a 10.8 GB backup took 35 s, almost all of it system
/// time, where one reader feeding 13 workers took 3 s.
pub fn read_each(path: &Path, indexes: &[usize], each: &Each<'_>, progress: &mut dyn FnMut(usize, u64) -> Result<()>) -> Result<()> {
    let workers = std::thread::available_parallelism().map_or(2, usize::from).saturating_sub(1).clamp(1, indexes.len().max(1));
    let stopped = AtomicBool::new(false);
    let mut failure: Option<Error> = None;
    let handle = |update: Update, failure: &mut Option<Error>, progress: &mut dyn FnMut(usize, u64) -> Result<()>| {
        let result = match update {
            Update::Bytes(position, bytes) if failure.is_none() => progress(position, bytes),
            Update::Bytes(..) => Ok(()),
            Update::Failed(error) => Err(error),
        };
        if let Err(error) = result {
            stopped.store(true, Ordering::Release);
            // A worker's Cancelled only echoes the stop that caused it.
            if failure.is_none() || matches!(failure, Some(Error::Cancelled)) {
                *failure = Some(error);
            }
        }
    };
    let (jobs, queue) = mpsc::sync_channel::<Job>(workers * 2);
    let queue = Mutex::new(queue);
    let (updates, received) = mpsc::channel::<Update>();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let (queue, updates, stopped) = (&queue, updates.clone(), &stopped);
            scope.spawn(move || loop {
                let job = match queue.lock() {
                    Ok(queue) => queue.recv(),
                    Err(_) => return,
                };
                let Ok(job) = job else { return };
                if stopped.load(Ordering::Acquire) {
                    continue;
                }
                let result = run(&job, each, &mut |bytes| {
                    if stopped.load(Ordering::Acquire) {
                        return Err(Error::Cancelled);
                    }
                    updates.send(Update::Bytes(job.position, bytes)).map_err(|_| Error::Cancelled)
                });
                if let Err(error) = result {
                    stopped.store(true, Ordering::Release);
                    let _ = updates.send(Update::Failed(error));
                }
            });
        }
        drop(updates);
        let read = (|| -> Result<()> {
            let mut zip = open(path)?;
            for (position, &index) in indexes.iter().enumerate() {
                while let Ok(update) = received.try_recv() {
                    handle(update, &mut failure, progress);
                }
                if stopped.load(Ordering::Acquire) {
                    return Ok(());
                }
                let mut entry = zip.by_index_raw(index)?;
                let meta = Meta { size: entry.size(), unix_mode: entry.unix_mode() };
                let (method, crc) = (entry.compression(), entry.crc32());
                if entry.compressed_size() <= IN_MEMORY && matches!(method, CompressionMethod::Stored | CompressionMethod::Deflated) {
                    let mut data = Vec::with_capacity(usize::try_from(entry.compressed_size()).unwrap_or(0));
                    entry.read_to_end(&mut data)?;
                    if jobs.send(Job { position, meta, method, crc, data }).is_err() {
                        return Ok(());
                    }
                } else {
                    drop(entry);
                    let mut entry = zip.by_index(index)?;
                    each(position, meta, &mut entry, &mut |bytes| progress(position, bytes))?;
                }
            }
            Ok(())
        })();
        drop(jobs);
        if let Err(error) = read {
            stopped.store(true, Ordering::Release);
            if failure.is_none() || matches!(failure, Some(Error::Cancelled)) {
                failure = Some(error);
            }
        }
        for update in received {
            handle(update, &mut failure, progress);
        }
    });
    failure.map_or(Ok(()), Err)
}

fn run(job: &Job, each: &Each<'_>, report: &mut Report<'_>) -> Result<()> {
    let data = job.data.as_slice();
    let mut contents: Box<dyn Read + '_> = match job.method {
        CompressionMethod::Stored => Box::new(data),
        _ => Box::new(flate2::read::DeflateDecoder::new(data)),
    };
    let mut checked = Checked { inner: &mut contents, hasher: crc32fast::Hasher::new(), crc: job.crc, size: job.meta.size, read: 0 };
    each(job.position, job.meta, &mut checked, report)
}

/// Fails at the end of the contents unless their length and CRC match.
struct Checked<R> {
    inner: R,
    hasher: crc32fast::Hasher,
    crc: u32,
    size: u64,
    read: u64,
}

impl<R: Read> Read for Checked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        if count == 0 && !buf.is_empty() {
            if self.read != self.size || self.hasher.clone().finalize() != self.crc {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid checksum"));
            }
        } else {
            self.hasher.update(&buf[..count]);
            self.read += count as u64;
        }
        Ok(count)
    }
}

/// Copies all of `reader`, reporting each chunk.
pub fn copy(reader: &mut dyn Read, writer: &mut dyn Write, report: &mut Report<'_>) -> Result<u64> {
    let mut buffer = vec![0; 1024 * 1024];
    let mut copied = 0;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(copied);
        }
        writer.write_all(&buffer[..count])?;
        copied += count as u64;
        report(count as u64)?;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    /// Bytes that do not compress, so a large entry stays large.
    fn noise(length: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        (0..length)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                (state >> 56) as u8
            })
            .collect()
    }

    fn write(path: &Path, entries: &[(&str, &[u8], CompressionMethod)]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes, method) in entries {
            zip.start_file(*name, SimpleFileOptions::default().compression_method(*method)).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn contents(path: &Path, indexes: &[usize]) -> Result<(Vec<Vec<u8>>, u64)> {
        let found = Mutex::new(vec![Vec::new(); indexes.len()]);
        let mut reported = 0;
        read_each(
            path,
            indexes,
            &|position, _, reader, report| {
                let mut bytes = Vec::new();
                copy(reader, &mut bytes, report)?;
                found.lock().unwrap()[position] = bytes;
                Ok(())
            },
            &mut |_, bytes| {
                reported += bytes;
                Ok(())
            },
        )?;
        Ok((found.into_inner().unwrap(), reported))
    }

    #[test]
    fn every_entry_arrives_whole_in_memory_or_streamed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.zip");
        let large = noise(9 * 1024 * 1024, 1);
        let small: Vec<Vec<u8>> = (0..50).map(|i| noise(1000 + i * 37, i as u64)).collect();
        let mut entries: Vec<(String, Vec<u8>, CompressionMethod)> = small
            .iter()
            .enumerate()
            .map(|(i, bytes)| (format!("analysis/{i}.DAT"), bytes.clone(), if i % 2 == 0 { CompressionMethod::Deflated } else { CompressionMethod::Stored }))
            .collect();
        entries.insert(10, ("master.db".into(), large.clone(), CompressionMethod::Stored));
        let borrowed: Vec<(&str, &[u8], CompressionMethod)> = entries.iter().map(|(n, b, m)| (n.as_str(), b.as_slice(), *m)).collect();
        write(&path, &borrowed);
        let indexes: Vec<usize> = (0..entries.len()).collect();
        let (found, reported) = contents(&path, &indexes).unwrap();
        for (bytes, (_, expected, _)) in found.iter().zip(&entries) {
            assert_eq!(bytes, expected);
        }
        assert_eq!(reported, entries.iter().map(|(_, bytes, _)| bytes.len() as u64).sum::<u64>());
    }

    #[test]
    fn a_damaged_entry_fails_its_checksum_whichever_path_reads_it() {
        use std::io::{Seek, SeekFrom};
        let dir = tempfile::tempdir().unwrap();
        for size in [4096, 9 * 1024 * 1024] {
            let path = dir.path().join(format!("{size}.zip"));
            write(&path, &[("ok", b"fine", CompressionMethod::Stored), ("master.db", &noise(size, 7), CompressionMethod::Stored)]);
            let start = open(&path).unwrap().by_index_raw(1).unwrap().data_start();
            let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(start + 100)).unwrap();
            file.write_all(b"X").unwrap();
            drop(file);
            let error = contents(&path, &[0, 1]).unwrap_err();
            assert!(matches!(&error, Error::Io(e) if e.kind() == io::ErrorKind::InvalidData), "{size}: {error}");
        }
    }

    #[test]
    fn stopping_ends_the_work_with_the_stop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.zip");
        let entries: Vec<(String, Vec<u8>)> = (0..200).map(|i| (format!("analysis/{i}"), noise(4096, i))).collect();
        let borrowed: Vec<(&str, &[u8], CompressionMethod)> = entries.iter().map(|(n, b)| (n.as_str(), b.as_slice(), CompressionMethod::Deflated)).collect();
        write(&path, &borrowed);
        let handled = std::sync::atomic::AtomicUsize::new(0);
        let indexes: Vec<usize> = (0..entries.len()).collect();
        let result = read_each(
            &path,
            &indexes,
            &|_, _, reader, report| {
                handled.fetch_add(1, Ordering::Relaxed);
                copy(reader, &mut io::sink(), report).map(|_| ())
            },
            &mut |_, _| Err(Error::Cancelled),
        );
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(handled.load(Ordering::Relaxed) < entries.len());
    }

    #[test]
    fn entries_map_to_the_part_they_restore() {
        assert_eq!(classify("master.db"), Some(Entry::Database));
        assert_eq!(classify("master.db-wal"), Some(Entry::DatabaseWal));
        assert_eq!(classify("analysis/"), Some(Entry::Analysis(PathBuf::new())));
        assert_eq!(classify("analysis/P016/0000/ANLZ0000.DAT"), Some(Entry::Analysis(PathBuf::from("P016/0000/ANLZ0000.DAT"))));
        assert_eq!(classify("artwork/abc/artwork_m.jpg"), Some(Entry::Artwork(PathBuf::from("abc/artwork_m.jpg"))));
        assert_eq!(classify("masterPlaylists6.xml"), Some(Entry::LibraryFile("masterPlaylists6.xml")));
        assert_eq!(classify("summary.json"), Some(Entry::Other));
        assert_eq!(classify("restore-rekordbox.sh"), Some(Entry::Other));
        assert_eq!(classify("master.db/inner"), Some(Entry::Other));
        assert_eq!(classify("../escape"), None);
        assert_eq!(classify("analysis/../../escape"), None);
        assert_eq!(classify("/etc/passwd"), None);
    }
}
