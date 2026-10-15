#![allow(clippy::unwrap_used, clippy::expect_used)]
use aes::{
    cipher::{BlockEncrypt, KeyInit},
    Aes128,
};
use rbl_ncm::{convert_file, Error};
use std::{fs, path::Path};

fn fixture(audio: &[u8]) -> Vec<u8> {
    let secret = b"local-converter-test-key";
    let mut key = b"neteasecloudmusic".to_vec();
    key.extend(secret);
    let padding = 16 - key.len() % 16;
    key.extend(vec![padding as u8; padding]);
    let cipher = Aes128::new(b"hzHRAmso5kInbaxW".into());
    for block in key.chunks_exact_mut(16) {
        cipher.encrypt_block(block.into());
    }
    for byte in &mut key {
        *byte ^= 0x64;
    }
    let mut table = std::array::from_fn::<_, 256, _>(|i| i as u8);
    let mut last = 0u8;
    for i in 0..256 {
        last = last
            .wrapping_add(table[i])
            .wrapping_add(secret[i % secret.len()]);
        table.swap(i, last as usize);
    }
    let mut data = b"CTENFDAM\0\0".to_vec();
    data.extend((key.len() as u32).to_le_bytes());
    data.extend(key);
    data.extend(7u32.to_le_bytes());
    data.extend(b"ignored");
    data.extend([0; 5]);
    data.extend(12u32.to_le_bytes());
    data.extend(4u32.to_le_bytes());
    data.extend(b"art!padding!");
    for (i, &byte) in audio.iter().enumerate() {
        let j = ((i + 1) & 255) as u8;
        let index =
            table[j as usize].wrapping_add(table[table[j as usize].wrapping_add(j) as usize]);
        data.push(byte ^ table[index as usize]);
    }
    data
}

fn write_fixture(root: &Path, audio: &[u8]) -> std::path::PathBuf {
    let source = root.join("网易云测试.ncm");
    fs::write(&source, fixture(audio)).unwrap();
    source
}

#[test]
fn restores_audio_across_chunks_and_preserves_the_source() {
    let root = tempfile::tempdir().unwrap();
    let mut audio = b"fLaC".to_vec();
    audio.extend((0..150_003).map(|i| (i % 251) as u8));
    let source = write_fixture(root.path(), &audio);
    let original = fs::read(&source).unwrap();
    let result = convert_file(&source, root.path()).unwrap();
    assert_eq!(result.format, "flac");
    assert_eq!(result.bytes, audio.len() as u64);
    assert_eq!(fs::read(result.path).unwrap(), audio);
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn detects_mp3_with_and_without_id3() {
    for audio in [
        b"ID3\x04\0\0\0\0\0\0fixture".as_slice(),
        b"\xff\xfb\x90\x64fixture".as_slice(),
    ] {
        let root = tempfile::tempdir().unwrap();
        let source = write_fixture(root.path(), audio);
        let result = convert_file(&source, root.path()).unwrap();
        assert_eq!(result.format, "mp3");
        assert_eq!(fs::read(result.path).unwrap(), audio);
    }
}

#[test]
fn never_overwrites_an_existing_file_or_a_concurrent_output() {
    let root = tempfile::tempdir().unwrap();
    let source = write_fixture(root.path(), b"fLaCtest");
    let existing = root.path().join("网易云测试.flac");
    fs::write(&existing, b"keep me").unwrap();
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| convert_file(&source, root.path()).unwrap());
        let b = scope.spawn(|| convert_file(&source, root.path()).unwrap());
        [a.join().unwrap().path, b.join().unwrap().path]
    });
    assert_ne!(results[0], results[1]);
    assert_eq!(fs::read(existing).unwrap(), b"keep me");
    for path in results {
        assert_eq!(fs::read(path).unwrap(), b"fLaCtest");
    }
}

#[test]
fn rejects_invalid_headers_and_unbounded_lengths_without_leaving_output() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("bad.ncm");
    for bytes in [
        b"ordinary mp3".to_vec(),
        [b"CTENFDAM\0\0".as_slice(), &u32::MAX.to_le_bytes()].concat(),
        fixture(b"unknown audio"),
    ] {
        fs::write(&source, bytes).unwrap();
        assert!(convert_file(&source, root.path()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn rejects_truncated_containers_and_corrupt_keys() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("bad.ncm");
    let good = fixture(b"fLaCtest");
    for end in 0..good.len() - 8 {
        fs::write(&source, &good[..end]).unwrap();
        assert!(
            convert_file(&source, root.path()).is_err(),
            "accepted truncation at {end}"
        );
    }
    let mut bad = good;
    bad[14] ^= 0xff;
    fs::write(&source, bad).unwrap();
    assert!(matches!(
        convert_file(&source, root.path()),
        Err(Error::Damaged)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn rejects_missing_output_folders() {
    let root = tempfile::tempdir().unwrap();
    let source = write_fixture(root.path(), b"fLaCtest");
    assert!(matches!(
        convert_file(&source, &root.path().join("missing")),
        Err(Error::OutputFolder)
    ));
}
