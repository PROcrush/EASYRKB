//! Builds a disposable USB export for the opt-in XDJ-AZ AtEmu test.
//!
//! ```text
//! cargo run -q -p rbl-export --example xdj_az_fixture -- /path/to/stick
//! ```
//!
//! The destination must be an empty or disposable directory. The source is a
//! generated 220 Hz WAV, so this never opens an installed rekordbox library.
#![allow(
    clippy::pedantic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used
)]

use std::f32::consts::TAU;
use std::path::Path;

const RATE: u32 = 44_100;
const SECONDS: u32 = 30;
const BPM: f32 = 120.0;

fn main() {
    let Some(destination) = std::env::args().nth(1) else {
        eprintln!("usage: xdj_az_fixture <disposable-usb-directory>");
        std::process::exit(2);
    };
    let destination = Path::new(&destination);
    std::fs::create_dir_all(destination).expect("create USB fixture directory");

    let source = tempfile::tempdir().expect("create source directory");
    let wav = source.path().join("XDJ-AZ 220Hz.wav");
    write_tone(&wav);

    let audio = rbl_audio::decode_mono(&wav, None).expect("decode generated tone");
    let analysed = rbl_analysis::analyse(&audio.samples, audio.sample_rate);
    let beats: Vec<rbl_anlz::Beat> = analysed
        .tempo
        .beats
        .iter()
        .map(|beat| rbl_anlz::Beat {
            beat_number: beat.beat_number,
            tempo_x100: beat.tempo_x100,
            time_ms: beat.time_ms,
        })
        .collect();
    let columns: Vec<rbl_anlz::BandColumn> = analysed
        .waveform
        .columns
        .iter()
        .map(|column| rbl_anlz::BandColumn {
            low: column.low,
            mid: column.mid,
            high: column.high,
            peak: column.peak,
        })
        .collect();
    let authored = rbl_anlz::author_with_overview(
        &wav.to_string_lossy(),
        &beats,
        &columns,
        analysed.waveform.overview.as_slice().try_into().ok(),
        rbl_anlz::Existing {
            dat: None,
            ext: None,
            two_ex: None,
        },
    );

    let track = rbl_export::SourceTrack {
        id: 1,
        source_path: wav,
        title: "XDJ-AZ 220Hz".into(),
        artist: "RBXport fixture".into(),
        album: "AtEmu".into(),
        genre: "Test tone".into(),
        key: "Am".into(),
        bpm_x100: (BPM * 100.0) as u32,
        duration_sec: SECONDS as u16,
        sample_rate: RATE,
        analysis: vec![
            ("DAT".into(), authored.dat),
            ("EXT".into(), authored.ext),
            ("2EX".into(), authored.two_ex),
        ],
        ..Default::default()
    };
    let playlist = rbl_export::SourcePlaylist {
        id: 1,
        name: "XDJ-AZ fixture".into(),
        track_indices: vec![0],
        ..Default::default()
    };

    let report = rbl_export::export(destination, &[track], &[playlist]).expect("export fixture");
    let verified = rbl_export::verify(destination).expect("verify fixture");
    assert!(
        verified.is_ok(),
        "fixture verification failed: {verified:#?}"
    );
    println!(
        "{}",
        serde_json::json!({
            "destination": destination,
            "title": "XDJ-AZ 220Hz",
            "frequencyHz": 220,
            "tracks": report.tracks,
            "playlists": report.playlists,
        })
    );
}

fn write_tone(path: &Path) {
    let frames = RATE * SECONDS;
    let beat_frames = RATE / 2;
    let mut data = Vec::with_capacity(frames as usize * 4);
    for frame in 0..frames {
        let t = frame as f32 / RATE as f32;
        let beat = frame % beat_frames;
        let click = if beat < 500 {
            0.5 * (1.0 - beat as f32 / 500.0)
        } else {
            0.0
        };
        let sample = (0.25 * (TAU * 220.0 * t).sin() + click).clamp(-1.0, 1.0);
        let pcm = (sample * 32_000.0) as i16;
        data.extend_from_slice(&pcm.to_le_bytes());
        data.extend_from_slice(&pcm.to_le_bytes());
    }
    let data_len = u32::try_from(data.len()).expect("WAV length fits u32");
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&data);
    std::fs::write(path, wav).expect("write generated tone");
}
