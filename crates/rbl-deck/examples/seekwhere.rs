//! Where the time in a long seek goes: the demuxer, or the decoding after it.
//! READ-ONLY.
//!
//! `jumpcost` says a jump across a three-hour file hesitates for seconds. This
//! splits that time in two — `format.seek` against the decoding the deck then
//! does to land frame-exact — so the fix addresses whichever one it is rather
//! than whichever is easier to change.
//!
//! `cargo run --release -p rbl-deck --example seekwhere -- <audio file>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

fn main() {
    let Some(path) = std::env::args().nth(1).map(std::path::PathBuf::from) else {
        println!("usage: seekwhere <audio file>");
        return;
    };

    for mode in [SeekMode::Accurate, SeekMode::Coarse] {
        println!("== {mode:?}");
        for seconds in [10.0_f64, 60.0, 600.0, 3600.0, 10_000.0] {
            let file = std::fs::File::open(&path).unwrap();
            let stream = MediaSourceStream::new(Box::new(file), Default::default());
            let mut hint = Hint::new();
            if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                hint.with_extension(extension);
            }
            let probed = symphonia::default::get_probe()
                .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
                .unwrap();
            let mut format = probed.format;
            let track = format.tracks().first().unwrap();
            let track_id = track.id;
            let rate = track.codec_params.sample_rate.unwrap_or(44_100);
            let total = track.codec_params.n_frames.unwrap_or(0);
            if u64::from(rate) * (seconds as u64) > total {
                continue;
            }
            let mut decoder = symphonia::default::get_codecs()
                .make(&track.codec_params, &DecoderOptions::default())
                .unwrap();

            let t0 = Instant::now();
            let landed = format
                .seek(mode, SeekTo::Time { time: Time::from(seconds), track_id: Some(track_id) })
                .unwrap();
            let seek_ms = t0.elapsed().as_secs_f64() * 1000.0;

            // What the deck does next: decode from where the demuxer landed
            // until it reaches the frame that was actually asked for.
            let wanted = (seconds * f64::from(rate)) as u64;
            let mut at = landed.actual_ts;
            let t1 = Instant::now();
            decoder.reset();
            let mut packets = 0_u32;
            while at < wanted {
                let Ok(packet) = format.next_packet() else { break };
                if packet.track_id() != track_id {
                    continue;
                }
                match decoder.decode(&packet) {
                    Ok(buf) => {
                        at = packet.ts() + buf.frames() as u64;
                        packets += 1;
                    }
                    Err(_) => break,
                }
            }
            let discard_ms = t1.elapsed().as_secs_f64() * 1000.0;
            let off = wanted as i64 - landed.actual_ts as i64;
            println!(
                "  to {seconds:>7.0} s   seek {seek_ms:>8.1} ms   landed {off:>+10} frames   \
                 discard {discard_ms:>7.1} ms over {packets} packets",
            );
        }
    }
}
