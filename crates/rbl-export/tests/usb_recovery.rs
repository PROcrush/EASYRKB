#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]
use rbl_anlz::{cues::ExportCue, Anlz, AnlzBuilder, Beat};
use rbl_export::{
    device_library::{self, Format},
    recovery::{RecoveryError, Session},
    SourcePlaylist, SourceTrack,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn fixture(root: &Path) {
    let originals = tempfile::tempdir().unwrap();
    let audio = originals.path().join("歌曲 #1.mp3");
    fs::write(&audio, vec![7; 128]).unwrap();
    let beats = [
        Beat {
            beat_number: 1,
            tempo_x100: 12800,
            time_ms: 125,
        },
        Beat {
            beat_number: 2,
            tempo_x100: 13000,
            time_ms: 595,
        },
    ];
    let cues = [
        ExportCue {
            kind: 0,
            time_ms: 1500,
            loop_time_ms: Some(2500),
            comment: "循环 & 测试".into(),
            ..Default::default()
        },
        ExportCue {
            kind: 1,
            time_ms: 500,
            ..Default::default()
        },
    ];
    let mut builder = AnlzBuilder::new();
    builder.path("/歌曲 #1.mp3").beat_grid(&beats);
    for section in rbl_anlz::cues::sections(&cues, true) {
        builder.copy_section(&section);
    }
    builder.copy_section(&rbl_anlz::Section::new(b"ZZZZ", vec![1, 2], vec![3, 4]));
    let tracks = [SourceTrack {
        metadata: rbl_core::ExportMetadata {
            hot_cue_auto_load: true,
            track_number: 7,
            disc_number: 2,
            bit_depth: 24,
            play_count: 9,
            ..Default::default()
        },
        id: 1,
        source_path: audio,
        title: "歌曲 & 测试".into(),
        artist: "艺术家".into(),
        album: "演出专辑".into(),
        bpm_x100: 12800,
        rating: 4,
        color_id: 3,
        duration_sec: 180,
        analysis: vec![("DAT".into(), builder.finish())],
        ..Default::default()
    }];
    let playlists = [
        SourcePlaylist {
            id: 10,
            name: "演出".into(),
            folder: true,
            ..Default::default()
        },
        SourcePlaylist {
            id: 11,
            parent_id: 10,
            name: "歌单 & 顺序".into(),
            track_indices: vec![0, 0],
            ..Default::default()
        },
        SourcePlaylist {
            id: 12,
            name: "空歌单".into(),
            ..Default::default()
        },
    ];
    rbl_export::export(root, &tracks, &playlists).unwrap();
}
fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}
fn diagnose(root: &Path, cache: &Path) -> Session {
    Session::diagnose(root, cache, &|| false, &mut |_, _, _| {}).unwrap()
}

#[test]
fn diagnosis_is_read_only_and_isolates_each_broken_database() {
    for broken in ["export.pdb", "exportLibrary.db"] {
        let source = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        fixture(source.path());
        fs::write(
            source.path().join(format!("PIONEER/rekordbox/{broken}")),
            b"broken",
        )
        .unwrap();
        let before = inventory(source.path());
        let session = diagnose(source.path(), cache.path());
        assert_eq!(session.report.sources.len(), 2);
        assert_eq!(
            session
                .report
                .sources
                .iter()
                .filter(|s| s.status == "unreadable")
                .count(),
            1
        );
        let readable = session
            .report
            .sources
            .iter()
            .find(|s| s.status != "unreadable")
            .unwrap();
        assert_eq!(readable.tracks, 1);
        assert_eq!(readable.playlist_entries, 2);
        assert_eq!(readable.cue_tracks, 1);
        assert_eq!(readable.beat_grid_tracks, 1);
        assert_eq!(inventory(source.path()), before);
    }
}
#[test]
fn new_drive_restores_order_empty_playlists_cues_grid_and_unknown_analysis() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    fixture(source.path());
    let before = inventory(source.path());
    let session = diagnose(source.path(), cache.path());
    let result = session
        .restore_to_empty("standard-pdb", target.path(), &|| false, &mut |_, _, _| {})
        .unwrap();
    assert!(result.verified);
    assert_eq!(result.tracks, 1);
    let recovered = device_library::read(target.path(), Format::DeviceLibrary).unwrap();
    assert_eq!(
        recovered
            .nodes
            .iter()
            .find(|n| n.name == "歌单 & 顺序")
            .unwrap()
            .tracks
            .len(),
        2
    );
    assert!(recovered
        .nodes
        .iter()
        .any(|n| n.name == "空歌单" && n.tracks.is_empty()));
    let analysis = Anlz::read(
        &target
            .path()
            .join(recovered.tracks[0].analysis_path.trim_start_matches('/')),
    )
    .unwrap();
    assert_eq!(analysis.beat_grid().unwrap()[1].tempo_x100, 13000);
    assert_eq!(analysis.cue_entries().len(), 2);
    assert_eq!(analysis.section(b"ZZZZ").unwrap().payload, vec![3, 4]);
    assert_eq!(inventory(source.path()), before);
}
#[test]
fn xml_preserves_original_paths_loops_hot_cues_and_variable_tempo() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fixture(source.path());
    let session = diagnose(source.path(), cache.path());
    let path = cache.path().join("recover.xml");
    session
        .write_xml("standard-one", &path, &|| false, &mut |_, _, _| {})
        .unwrap();
    let xml = fs::read_to_string(&path).unwrap();
    assert!(xml.contains("Name=\"歌曲 &amp; 测试\""));
    assert!(xml.contains("Type=\"4\" Start=\"1.500\" Num=\"-1\" End=\"2.500\""));
    assert!(xml.contains("Type=\"0\" Start=\"0.500\" Num=\"0\""));
    assert!(xml.contains("Bpm=\"130.00\""));
    assert_eq!(xml.matches("<TEMPO ").count(), 2);
    assert!(xml.contains("%23"));
    assert!(xml.contains("file://localhost/"));
    assert_eq!(xml.matches("<TRACK Key=\"1\"/>").count(), 2);
    assert!(session
        .write_xml("standard-one", &path, &|| false, &mut |_, _, _| {})
        .is_err());
}
#[test]
fn stale_sources_nonempty_targets_source_targets_and_cancel_are_rejected() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    fixture(source.path());
    let session = diagnose(source.path(), cache.path());
    fs::write(target.path().join("personal.txt"), b"keep").unwrap();
    assert!(matches!(
        session.restore_to_empty("standard-pdb", target.path(), &|| false, &mut |_, _, _| {}),
        Err(RecoveryError::Destination)
    ));
    assert_eq!(
        fs::read(target.path().join("personal.txt")).unwrap(),
        b"keep"
    );
    assert!(matches!(
        session.restore_to_empty("standard-pdb", source.path(), &|| false, &mut |_, _, _| {}),
        Err(RecoveryError::Destination)
    ));
    assert!(matches!(
        session.write_xml(
            "standard-pdb",
            &cache.path().join("cancel.xml"),
            &|| true,
            &mut |_, _, _| {}
        ),
        Err(RecoveryError::Cancelled)
    ));
    assert!(!cache.path().join("cancel.xml").exists());
    fs::write(
        source.path().join("PIONEER/rekordbox/export.pdb"),
        b"changed",
    )
    .unwrap();
    assert!(matches!(
        session.write_xml(
            "standard-pdb",
            &cache.path().join("stale.xml"),
            &|| false,
            &mut |_, _, _| {}
        ),
        Err(RecoveryError::Changed)
    ));
}
#[test]
fn missing_audio_is_kept_in_xml_and_skipped_only_in_new_drive_recovery() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fixture(source.path());
    let library = device_library::read(source.path(), Format::DeviceLibrary).unwrap();
    fs::remove_file(
        source
            .path()
            .join(library.tracks[0].path.trim_start_matches('/')),
    )
    .unwrap();
    let session = diagnose(source.path(), cache.path());
    assert_eq!(session.report.sources[0].missing_audio, 1);
    let preview = session.preview("standard-pdb", 0, 100).unwrap();
    assert!(!preview.tracks[0].audio_present);
    let outcome = session
        .write_xml(
            "standard-pdb",
            &cache.path().join("missing.xml"),
            &|| false,
            &mut |_, _, _| {},
        )
        .unwrap();
    assert_eq!(outcome.tracks, 1);
    assert_eq!(outcome.skipped_audio, 1);
}
#[cfg(unix)]
#[test]
fn snapshot_cache_and_symlinks_cannot_write_into_the_source() {
    let source = tempfile::tempdir().unwrap();
    fixture(source.path());
    let before = inventory(source.path());
    assert!(matches!(
        Session::diagnose(
            source.path(),
            &source.path().join("cache"),
            &|| false,
            &mut |_, _, _| {}
        ),
        Err(RecoveryError::Destination)
    ));
    assert_eq!(inventory(source.path()), before);
    let cache = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(source.path(), cache.path().join("linked")).unwrap();
    assert!(matches!(
        Session::diagnose(
            source.path(),
            &cache.path().join("linked/new-cache"),
            &|| false,
            &mut |_, _, _| {}
        ),
        Err(RecoveryError::Destination)
    ));
}

#[test]
fn backup_failure_or_cancel_aborts_original_repair_before_any_usb_changes() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fixture(source.path());
    fs::write(
        source.path().join("PIONEER/rekordbox/export.pdb"),
        b"damaged",
    )
    .unwrap();
    let session = diagnose(source.path(), cache.path());
    let before = inventory(source.path());
    let blocked = cache.path().join("not-a-directory");
    fs::write(&blocked, b"blocked").unwrap();
    assert!(session
        .repair_current("standard-one", &blocked, &|| false, &mut |_, _, _| {})
        .is_err());
    assert_eq!(inventory(source.path()), before);
    for stop_at in ["backup", "database"] {
        let stopped = std::cell::Cell::new(false);
        assert!(matches!(
            session.repair_current(
                "standard-one",
                &cache.path().join("backups"),
                &|| stopped.get(),
                &mut |stage, _, _| {
                    if stage == stop_at {
                        stopped.set(true);
                    }
                }
            ),
            Err(RecoveryError::Cancelled)
        ));
        assert_eq!(inventory(source.path()), before);
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source.path(), cache.path().join("usb-link")).unwrap();
        assert!(matches!(
            session.repair_current(
                "standard-one",
                &cache.path().join("usb-link/new-backups"),
                &|| false,
                &mut |_, _, _| {}
            ),
            Err(RecoveryError::Destination)
        ));
        assert_eq!(inventory(source.path()), before);
    }
}

#[test]
fn legacy_cues_and_signed_grid_offset_are_exported_to_xml() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fixture(source.path());
    let library = device_library::read(source.path(), Format::DeviceLibrary).unwrap();
    let path = source
        .path()
        .join(library.tracks[0].analysis_path.trim_start_matches('/'));
    let mut analysis = Anlz::read(&path).unwrap();
    analysis.sections.retain(|s| !s.is_cue_list());
    analysis.sections.extend(rbl_anlz::cues::sections(
        &[
            ExportCue {
                kind: 0,
                time_ms: 1500,
                loop_time_ms: Some(2500),
                ..Default::default()
            },
            ExportCue {
                kind: 1,
                time_ms: 500,
                ..Default::default()
            },
        ],
        false,
    ));
    fs::write(&path, analysis.with_grid_offset(25).unwrap()).unwrap();
    let session = diagnose(source.path(), cache.path());
    assert_eq!(
        session.preview("standard-pdb", 0, 100).unwrap().tracks[0].cues,
        2
    );
    let target = cache.path().join("legacy.xml");
    session
        .write_xml("standard-pdb", &target, &|| false, &mut |_, _, _| {})
        .unwrap();
    let xml = fs::read_to_string(target).unwrap();
    assert!(xml.contains("Inizio=\"0.150\""));
    assert!(xml.contains("End=\"2.500\""));
}
#[test]
fn cancelling_staged_recovery_never_publishes_partial_audio_or_databases() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    fixture(source.path());
    fs::create_dir(source.path().join(".rbxport-publication")).unwrap();
    fs::write(
        source.path().join(".rbxport-publication/sentinel"),
        b"untouched",
    )
    .unwrap();
    let before = inventory(source.path());
    let session = diagnose(source.path(), cache.path());
    assert!(session.report.pending_publication);
    let stopped = std::cell::Cell::new(false);
    let result = session.restore_to_empty(
        "standard-one",
        target.path(),
        &|| stopped.get(),
        &mut |stage, _, _| {
            if stage == "copying" {
                stopped.set(true);
            }
        },
    );
    assert!(
        matches!(
            result,
            Err(RecoveryError::Cancelled
                | RecoveryError::Export(rbl_export::ExportError::Cancelled))
        ),
        "{result:?}"
    );
    assert!(!target.path().join("PIONEER").exists());
    assert!(!target.path().join("Contents").exists());
    assert_eq!(inventory(source.path()), before);
}

#[test]
fn in_place_repair_rebuilds_the_bad_database_after_verified_backup_only() {
    for (broken, selected) in [
        ("export.pdb", "standard-one"),
        ("exportLibrary.db", "standard-pdb"),
    ] {
        let source = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        fixture(source.path());
        let broken_path = PathBuf::from(format!("PIONEER/rekordbox/{broken}"));
        fs::write(
            source.path().join(&broken_path),
            b"corrupt original database",
        )
        .unwrap();
        let before = inventory(source.path());
        let session = diagnose(source.path(), cache.path());
        let outcome = session
            .repair_current(
                selected,
                &cache.path().join("backups"),
                &|| false,
                &mut |_, _, _| {},
            )
            .unwrap();
        assert_eq!(outcome.kind, "repair");
        assert!(outcome.verified);
        let backup = PathBuf::from(outcome.backup_path.unwrap());
        assert_eq!(
            fs::read(backup.join(&broken_path)).unwrap(),
            b"corrupt original database"
        );
        assert!(backup.join("backup.json").is_file());
        let after = inventory(source.path());
        for (path, bytes) in &before {
            if path != &broken_path {
                assert_eq!(
                    after.get(path),
                    Some(bytes),
                    "Unexpected change to {}",
                    path.display()
                );
            }
        }
        assert_eq!(
            device_library::read(source.path(), Format::DeviceLibrary)
                .unwrap()
                .tracks,
            device_library::read(source.path(), Format::OneLibrary)
                .unwrap()
                .tracks
        );
        assert!(rbl_export::verify_databases(source.path()).unwrap().is_ok());
    }
}
#[test]
fn original_repair_refuses_unchanged_libraries_stale_snapshots_and_unfinished_publication() {
    let source = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fixture(source.path());
    let session = diagnose(source.path(), cache.path());
    let before = inventory(source.path());
    assert!(!session.report.source_disagreement);
    assert!(matches!(
        session.repair_current(
            "standard-one",
            &cache.path().join("backup"),
            &|| false,
            &mut |_, _, _| {}
        ),
        Err(RecoveryError::Healthy)
    ));
    assert_eq!(inventory(source.path()), before);
    fs::create_dir(source.path().join(".rbxport-publication")).unwrap();
    fs::write(source.path().join(".rbxport-publication/sentinel"), b"keep").unwrap();
    let pending = inventory(source.path());
    assert!(session
        .repair_current(
            "standard-one",
            &cache.path().join("backup"),
            &|| false,
            &mut |_, _, _| {}
        )
        .is_err());
    assert_eq!(inventory(source.path()), pending);
    fs::remove_dir_all(source.path().join(".rbxport-publication")).unwrap();
    fs::write(
        source.path().join("PIONEER/rekordbox/exportLibrary.db"),
        b"changed after diagnosis",
    )
    .unwrap();
    let stale = inventory(source.path());
    assert!(matches!(
        session.repair_current(
            "standard-one",
            &cache.path().join("backup"),
            &|| false,
            &mut |_, _, _| {}
        ),
        Err(RecoveryError::Changed)
    ));
    assert_eq!(inventory(source.path()), stale);
}
