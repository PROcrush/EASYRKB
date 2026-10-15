//! Extract the original audio from an NCM container without re-encoding.
//! Container layout reference: https://github.com/iqiziqi/ncmdump.rs
use aes::{
    cipher::{BlockDecrypt, KeyInit},
    Aes128,
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("This file is not a valid NCM file.")]
    Invalid,
    #[error("The NCM file is damaged or incomplete.")]
    Damaged,
    #[error("The NCM audio format is not supported. Only MP3 and FLAC are supported.")]
    Unsupported,
    #[error("Choose an existing output folder.")]
    OutputFolder,
    #[error("Could not find an available output filename.")]
    Filename,
    #[error("Could not read or write the audio file: {0}")]
    Io(#[from] std::io::Error),
}

pub struct Converted {
    pub path: PathBuf,
    pub format: &'static str,
    pub bytes: u64,
}

fn number(reader: &mut File) -> Result<u32, Error> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes).map_err(|_| Error::Damaged)?;
    Ok(u32::from_le_bytes(bytes))
}

fn skip(reader: &mut File, bytes: u64, length: u64) -> Result<(), Error> {
    let end = reader
        .stream_position()?
        .checked_add(bytes)
        .ok_or(Error::Damaged)?;
    if end > length {
        return Err(Error::Damaged);
    }
    reader.seek(SeekFrom::Start(end))?;
    Ok(())
}

fn key_stream(reader: &mut File, length: u64) -> Result<[u8; 256], Error> {
    let mut header = [0; 10];
    reader.read_exact(&mut header).map_err(|_| Error::Invalid)?;
    if &header[..8] != b"CTENFDAM" {
        return Err(Error::Invalid);
    }
    let size = number(reader)? as usize;
    if size == 0
        || size > 65536
        || size % 16 != 0
        || reader.stream_position()? + size as u64 > length
    {
        return Err(Error::Damaged);
    }
    let mut key = vec![0; size];
    reader.read_exact(&mut key).map_err(|_| Error::Damaged)?;
    for byte in &mut key {
        *byte ^= 0x64;
    }
    let cipher = Aes128::new(b"hzHRAmso5kInbaxW".into());
    for block in key.chunks_exact_mut(16) {
        cipher.decrypt_block(block.into());
    }
    let padding = usize::from(key[size - 1]);
    if padding == 0
        || padding > 16
        || !key[size - padding..]
            .iter()
            .all(|&b| usize::from(b) == padding)
    {
        return Err(Error::Damaged);
    }
    key.truncate(size - padding);
    let secret = key
        .strip_prefix(b"neteasecloudmusic")
        .filter(|key| !key.is_empty())
        .ok_or(Error::Damaged)?;
    let mut table = std::array::from_fn::<_, 256, _>(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j
            .wrapping_add(table[i])
            .wrapping_add(secret[i % secret.len()]);
        table.swap(i, usize::from(j));
    }
    let stream = std::array::from_fn(|i| {
        let j = (i + 1) as u8;
        table[usize::from(
            table[usize::from(j)]
                .wrapping_add(table[usize::from(j.wrapping_add(table[usize::from(j)]))]),
        )]
    });
    let metadata = u64::from(number(reader)?);
    skip(reader, metadata, length)?;
    skip(reader, 5, length)?; // CRC and reserved byte
    let cover_frame = u64::from(number(reader)?);
    let cover_size = u64::from(number(reader)?);
    if cover_size > cover_frame {
        return Err(Error::Damaged);
    }
    skip(reader, cover_frame, length)?;
    if reader.stream_position()? >= length {
        return Err(Error::Damaged);
    }
    Ok(stream)
}

fn format(bytes: &[u8]) -> Result<&'static str, Error> {
    if bytes.starts_with(b"fLaC") {
        return Ok("flac");
    }
    if bytes.starts_with(b"ID3")
        || (bytes.len() >= 4
            && bytes[0] == 0xff
            && bytes[1] & 0xe0 == 0xe0
            && bytes[1] & 0x18 != 0x08
            && bytes[1] & 0x06 != 0
            && bytes[2] & 0xf0 != 0xf0
            && bytes[2] & 0x0c != 0x0c)
    {
        return Ok("mp3");
    }
    Err(Error::Unsupported)
}

/// Publishes a completed file atomically, never overwriting a source or output.
/// Only the bounded key and one audio chunk are held in memory. Embedded audio
/// tags are preserved byte for byte; separate NCM metadata/artwork are skipped.
pub fn convert_file(source: &Path, directory: &Path) -> Result<Converted, Error> {
    if !directory.is_dir() {
        return Err(Error::OutputFolder);
    }
    let mut reader = File::open(source)?;
    let length = reader.metadata()?.len();
    let stream = key_stream(&mut reader, length)?;
    let remaining = length - reader.stream_position()?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    let mut chunk = [0; 64 * 1024];
    let mut bytes = 0u64;
    let mut detected = None;
    while bytes < remaining {
        let count = (remaining - bytes).min(chunk.len() as u64) as usize;
        reader
            .read_exact(&mut chunk[..count])
            .map_err(|_| Error::Damaged)?;
        for (i, byte) in chunk[..count].iter_mut().enumerate() {
            *byte ^= stream[((bytes & 255) as usize + i) & 255];
        }
        if detected.is_none() {
            detected = Some(format(&chunk[..count])?);
        }
        temporary.write_all(&chunk[..count])?;
        bytes += count as u64;
    }
    let format = detected.ok_or(Error::Damaged)?;
    temporary.as_file().sync_all()?;
    let stem = source.file_stem().ok_or(Error::Filename)?.to_string_lossy();
    // Leave room for the suffix on filesystems with a 255-byte name limit.
    let stem = stem
        .chars()
        .scan(0, |size, ch| {
            *size += ch.len_utf8();
            (*size <= 220).then_some(ch)
        })
        .collect::<String>();
    for suffix in 0..10_000 {
        let name = if suffix == 0 {
            format!("{stem}.{format}")
        } else {
            format!("{stem} ({suffix}).{format}")
        };
        let path = directory.join(name);
        match temporary.persist_noclobber(&path) {
            Ok(_) => {
                return Ok(Converted {
                    path,
                    format,
                    bytes,
                })
            }
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => temporary = e.file,
            Err(e) => return Err(Error::Io(e.error)),
        }
    }
    Err(Error::Filename)
}
