//! Capacity previews use the encoder output and actual retained device files.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rbl_export::{CompatibilityFormat, ExportOptions, Manifest, SourceTrack};

fn flac(dir: &std::path::Path) -> SourceTrack {
    let source = dir.join("source.flac");
    std::fs::write(&source, include_bytes!("../../rbl-audio/tests/fixtures/stereo-96k.flac")).unwrap();
    SourceTrack { id: 71, source_path: source, title: "Capacity".into(),
        // Deliberately stale: the header's quarter-second duration wins.
        duration_sec: 300, ..Default::default() }
}

#[test]
fn conversion_previews_follow_each_encoder_instead_of_source_size() {
    let src = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let track = flac(src.path());
    let original = std::fs::read(&track.source_path).unwrap();
    let tracks = [track];
    let unchanged = rbl_export::estimate::estimate(dest.path(), 0, &tracks, false, None);
    assert_eq!(unchanged.copy_bytes, u64::try_from(original.len()).unwrap());
    assert!(!unchanged.approximate);
    let mut sizes = Vec::new();
    for (format, encoder) in [
        (CompatibilityFormat::Wav, rbl_audio::compatibility::Format::Wav),
        (CompatibilityFormat::Aiff, rbl_audio::compatibility::Format::Aiff),
        (CompatibilityFormat::Mp3, rbl_audio::compatibility::Format::Mp3),
    ] {
        let before = rbl_export::estimate::estimate(dest.path(), 0, &tracks, false, Some(format));
        let output = src.path().join(format!("converted.{}", encoder.extension()));
        let actual = rbl_audio::compatibility::convert(&tracks[0].source_path, &output, encoder).unwrap();
        assert!(before.approximate);
        assert_eq!((before.tracks_new, before.tracks_changed), (1, 0));
        assert!(before.copy_bytes >= actual, "{format:?}: {} < {actual}", before.copy_bytes);
        assert!(before.copy_bytes - actual <= 8192, "duration must come from the audio header");
        sizes.push(before.copy_bytes);
    }
    assert_eq!(sizes[0], 44 + 11025 * 4);
    assert_eq!(sizes[1], 54 + 11025 * 4);
    assert!(sizes[2] < sizes[0]);
    assert_eq!(std::fs::read(&tracks[0].source_path).unwrap(), original);
    assert_eq!(std::fs::read_dir(dest.path()).unwrap().count(), 0, "preview writes nothing");
}

#[test]
fn a_reused_conversion_reports_the_size_actually_on_the_device() {
    let src = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let tracks = [flac(src.path())];
    let options = ExportOptions { compatibility: Some(CompatibilityFormat::Wav), ..Default::default() };
    rbl_export::export_with_options(dest.path(), &tracks, &[], &[], &options, &mut |_| {}).unwrap();
    let manifest = Manifest::load(dest.path()).unwrap();
    let entry = &manifest.tracks[0];
    let actual = std::fs::metadata(dest.path().join(entry.audio.trim_start_matches('/'))).unwrap().len();
    let again = rbl_export::estimate::estimate(dest.path(), manifest.db_id, &tracks, false, options.compatibility);
    assert_eq!((again.copy_bytes, again.tracks_kept, again.reuse_bytes), (0, 1, actual));
    assert!(!again.approximate, "existing bytes can be measured exactly");

    let mp3 = rbl_export::estimate::estimate(dest.path(), manifest.db_id, &tracks, false, Some(CompatibilityFormat::Mp3));
    assert_eq!((mp3.tracks_changed, mp3.tracks_kept, mp3.reuse_bytes), (1, 0, 0));
    assert_eq!(mp3.copy_bytes, 250 * 40 + 8192);
    assert!(mp3.approximate);
}

#[test]
fn choosing_conversion_keeps_compatible_audio_as_a_direct_copy() {
    let src = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let mut track = flac(src.path());
    let output = src.path().join("compatible.wav");
    let actual = rbl_audio::compatibility::convert(&track.source_path, &output, rbl_audio::compatibility::Format::Wav).unwrap();
    track.source_path = output;
    for format in [CompatibilityFormat::Wav, CompatibilityFormat::Aiff, CompatibilityFormat::Mp3] {
        let found = rbl_export::estimate::estimate(dest.path(), 0, &[track.clone()], false, Some(format));
        assert_eq!(found.copy_bytes, actual);
        assert!(!found.approximate);
    }
}
