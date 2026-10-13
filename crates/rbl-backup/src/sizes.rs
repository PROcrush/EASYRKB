//! Logical bytes of a library backup by kind. Only ANLZ section headers are
//! read; waveform payloads are skipped over rather than loaded.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSizes {
    /// Unix milliseconds when these were measured.
    pub updated_at: u64,
    pub track_count: u32,
    pub artwork: u64,
    pub vocals: u64,
    /// `master.db`, its WAL and the library files beside it.
    pub database: u64,
    pub waveforms: u64,
    pub cues: u64,
    pub beat_grids: u64,
    pub phrases: u64,
    pub other: u64,
}

impl BackupSizes {
    pub fn add(&mut self, other: &Self) {
        self.database += other.database;
        self.artwork += other.artwork;
        self.vocals += other.vocals;
        self.waveforms += other.waveforms;
        self.cues += other.cues;
        self.beat_grids += other.beat_grids;
        self.phrases += other.phrases;
        self.other += other.other;
    }

    pub fn total(&self) -> u64 {
        self.database + self.artwork + self.vocals + self.waveforms + self.cues + self.beat_grids + self.phrases + self.other
    }
}

/// Sizes, and how many files the two trees hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Measurement {
    pub sizes: BackupSizes,
    pub analysis_files: u64,
    pub artwork_files: u64,
}

fn regular_size(path: &Path) -> io::Result<u64> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Expected a regular backup file"));
    }
    Ok(meta.len())
}

/// The live library: the database with its WAL and library files, and both trees.
pub fn measure_paths(database: &Path, analysis: &Path, artwork: &Path) -> io::Result<Measurement> {
    let mut measured = Measurement::default();
    measured.sizes.database = regular_size(database)?;
    let optional = [crate::sidecar(database, "-wal")]
        .into_iter()
        .chain(database.parent().into_iter().flat_map(|root| crate::LIBRARY_FILES.map(|name| root.join(name))));
    for path in optional {
        match regular_size(&path) {
            Ok(bytes) => measured.sizes.database += bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    measure_tree(analysis, false, &mut measured, &mut || Ok(()))?;
    measure_tree(artwork, true, &mut measured, &mut || Ok(()))?;
    Ok(measured)
}

/// Adds a tree's files to `measured`. A missing tree adds nothing. `check`
/// runs before each entry, so a caller can stop a long walk.
pub fn measure_tree(
    path: &Path,
    is_artwork: bool,
    measured: &mut Measurement,
    check: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    match walk(path, is_artwork, measured, check) {
        Err(e) if e.kind() == io::ErrorKind::NotFound && !path.exists() => Ok(()),
        result => result,
    }
}

fn walk(
    path: &Path,
    is_artwork: bool,
    measured: &mut Measurement,
    check: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    check()?;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Symbolic links are not supported in backups"));
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            walk(&entry?.path(), is_artwork, measured, check)?;
        }
    } else if meta.is_file() {
        if is_artwork {
            measured.sizes.artwork += meta.len();
            measured.artwork_files += 1;
        } else {
            measured.sizes.add(&analysis_sizes(&mut fs::File::open(path)?, meta.len())?);
            measured.analysis_files += 1;
        }
    } else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported backup file"));
    }
    Ok(())
}

/// One analysis file of `length` bytes by section kind. A file that is not a
/// well-formed ANLZ file counts as other analysis in full.
pub fn analysis_sizes<R: Read + Seek>(file: &mut R, length: u64) -> io::Result<BackupSizes> {
    let unknown = || BackupSizes { other: length, ..Default::default() };
    if length < 12 {
        return Ok(unknown());
    }
    let mut header = [0; 12];
    file.read_exact(&mut header)?;
    let number = |bytes: &[u8]| u64::from(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    let mut at = number(&header[4..8]);
    if &header[..4] != b"PMAI" || at < 12 || at > length {
        return Ok(unknown());
    }
    let mut sizes = BackupSizes { other: at, ..Default::default() };
    while length - at >= 12 {
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut header)?;
        let header_len = number(&header[4..8]);
        let section_len = number(&header[8..12]);
        if header_len < 12 || section_len < header_len || section_len > length - at {
            return Ok(unknown());
        }
        match &header[..4] {
            b"PWAV" | b"PWV2" | b"PWV3" | b"PWV4" | b"PWV5" | b"PWV6" | b"PWV7" => sizes.waveforms += section_len,
            b"PCOB" | b"PCO2" => sizes.cues += section_len,
            b"PQTZ" | b"PQT2" => sizes.beat_grids += section_len,
            b"PSSI" => sizes.phrases += section_len,
            b"PVDI" => sizes.vocals += section_len,
            _ => sizes.other += section_len,
        }
        at += section_len;
    }
    sizes.other += length - at;
    Ok(sizes)
}

/// A reader that can only seek forward, by reading and discarding. Lets
/// [`analysis_sizes`] walk a compressed archive entry.
pub struct Forward<R> {
    inner: R,
    position: u64,
}

impl<R: Read> Forward<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, position: 0 }
    }
}

impl<R: Read> Read for Forward<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        self.position += count as u64;
        Ok(count)
    }
}

impl<R: Read> Seek for Forward<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let SeekFrom::Start(target) = position else {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Only absolute forward seeks are supported"));
        };
        if target < self.position {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Cannot seek backwards"));
        }
        let distance = target - self.position;
        let skipped = io::copy(&mut (&mut self.inner).take(distance), &mut io::sink())?;
        self.position += skipped;
        if skipped < distance {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(target)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn accounts_for_every_byte_in_database_wal_and_analysis_only() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("master.db");
        fs::write(&db, [0; 100]).unwrap();
        fs::write(dir.path().join("master.db-wal"), [0; 20]).unwrap();
        fs::write(dir.path().join("masterPlaylists6.xml"), [0; 10]).unwrap();
        fs::write(dir.path().join("music.mp3"), [0; 99]).unwrap();
        let anlz = dir.path().join("analysis");
        fs::create_dir(&anlz).unwrap();
        let bytes = crate::testing::anlz(&[b"PWV7", b"PCOB", b"PQTZ", b"PSSI", b"PVDI", b"PPTH"]);
        fs::write(anlz.join("ANLZ.2EX"), &bytes).unwrap();
        fs::write(anlz.join("unknown"), [0; 7]).unwrap();
        let artwork = dir.path().join("artwork");
        fs::create_dir_all(artwork.join("abc")).unwrap();
        fs::write(artwork.join("abc/artwork_m.jpg"), [0; 31]).unwrap();
        let measured = measure_paths(&db, &anlz, &artwork).unwrap();
        let sizes = &measured.sizes;
        assert_eq!((measured.analysis_files, measured.artwork_files), (2, 1));
        assert_eq!(sizes.artwork, 31);
        assert_eq!(sizes.vocals, 16);
        assert_eq!(sizes.database, 130);
        assert_eq!((sizes.waveforms, sizes.cues, sizes.beat_grids, sizes.phrases), (16, 16, 16, 16));
        assert_eq!(sizes.other, 12 + 16 + 7);
        assert_eq!(sizes.total() - sizes.database - sizes.artwork, bytes.len() as u64 + 7);
    }

    #[test]
    fn malformed_analysis_is_counted_as_other_and_missing_analysis_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("master.db");
        fs::write(&db, [0; 8]).unwrap();
        let measured = measure_paths(&db, &dir.path().join("absent"), &dir.path().join("no-artwork")).unwrap();
        assert_eq!(measured.sizes.database, 8);
        assert_eq!(measured.analysis_files, 0);
        let bytes = b"PMAI\x00\x00\x00\x0c\x00\x00\x00\x18PWAV\x00\x00\x00\x0c\xff\xff\xff\xff";
        let sizes = analysis_sizes(&mut io::Cursor::new(bytes), bytes.len() as u64).unwrap();
        assert_eq!(sizes.other, bytes.len() as u64);
        assert_eq!(sizes.waveforms, 0);
    }

    #[test]
    fn a_forward_only_reader_measures_the_same_as_a_file() {
        let bytes = crate::testing::anlz(&[b"PWV3", b"PCO2", b"PQT2"]);
        let seekable = analysis_sizes(&mut io::Cursor::new(&bytes), bytes.len() as u64).unwrap();
        let streamed = analysis_sizes(&mut Forward::new(bytes.as_slice()), bytes.len() as u64).unwrap();
        assert_eq!(seekable, streamed);
        assert_eq!((streamed.waveforms, streamed.cues, streamed.beat_grids), (16, 16, 16));
        let mut forward = Forward::new(bytes.as_slice());
        forward.seek(SeekFrom::Start(20)).unwrap();
        assert!(forward.seek(SeekFrom::Start(4)).is_err());
        assert!(forward.seek(SeekFrom::Start(10_000)).is_err());
    }
}
