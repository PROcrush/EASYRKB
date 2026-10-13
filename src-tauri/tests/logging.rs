//! The debug logger writes to a daily file in the named directory at the
//! level `LOG_LEVEL` asks for.
//!
//! Its own binary: the logger is process-global and installs once.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

#[test]
fn log_level_trace_reaches_the_file() {
    let dir = tempfile::tempdir().unwrap();
    // Set before the logger reads them; this test is the only one in the
    // binary, so nothing races the environment.
    std::env::set_var(rbxport_lib::logging::LOG_DIR_ENV, dir.path());
    std::env::set_var("LOG_LEVEL", "TRACE");
    std::env::remove_var("RUST_LOG");
    rbxport_lib::logging::install();

    // Under the crates' own targets: this test binary's target is not one
    // of ours, and the level applies to ours alone.
    tracing::trace!(target: "rbl_link::beacon", marker = "needle-trace", "a trace line");
    tracing::debug!(target: "rbl_nfs", marker = "needle-debug", "a debug line");
    tracing::debug!(marker = "needle-foreign", "a debug line from a crate that is not ours");
    tracing::warn!(
        target: "symphonia_bundle_mp3::layer3",
        marker = "needle-mp3-reservoir",
        "an expected seek warning"
    );
    tracing::warn!(
        target: "symphonia_bundle_mp3::demuxer",
        marker = "needle-mp3-demuxer",
        "an expected seek scan warning"
    );

    // The file is written off-thread; give it a moment.
    let mut text = String::new();
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(20));
        text = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("rbxport"))
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect();
        if text.contains("needle-trace") && text.contains("needle-debug") {
            break;
        }
    }
    assert!(text.contains("needle-trace"), "trace line missing from {text}");
    assert!(text.contains("needle-debug"), "debug line missing from {text}");
    assert!(text.contains("logging to stdout and a daily file"));
    assert!(!text.contains("needle-foreign"), "a dependency's debug line reached the file: {text}");
    assert!(
        !text.contains("needle-mp3-reservoir"),
        "Symphonia's expected seek warning reached the file: {text}"
    );
    assert!(
        !text.contains("needle-mp3-demuxer"),
        "Symphonia's expected seek scan warning reached the file: {text}"
    );
    assert!(!text.contains("\u{1b}["), "ANSI colour in the file: {text}");
}
