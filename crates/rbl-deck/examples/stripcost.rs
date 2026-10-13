//! What the channel strips cost, as a share of the time they have.
//!
//! The audio callback has one buffer's worth of wall clock to fill a buffer,
//! and everything else in the app has to fit in what is left. This runs both
//! decks' strips over a minute of audio and reports the fraction of realtime
//! they take.
//!
//! `cargo run --release -p rbl-deck --example stripcost`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

const RATE: u32 = 44_100;
const BUFFER: usize = 512;

fn main() {
    let mixer = rbl_deck::MixerSettings::default();
    let mut channels = [rbl_deck::Channel::new(RATE), rbl_deck::Channel::new(RATE)];
    // A minute of noise, which exercises every band rather than one of them.
    let seconds = 60;
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut buffer = vec![0.0_f32; BUFFER * 2];

    let buffers = seconds * RATE as usize / BUFFER;
    let start = Instant::now();
    for _ in 0..buffers {
        for sample in buffer.iter_mut() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *sample = (state >> 40) as f32 / 8_388_608.0 - 1.0;
        }
        for (i, channel) in channels.iter_mut().enumerate() {
            let settings = &mixer.channels[i];
            channel.process(&mut buffer, settings, mixer.curve(), 1.0);
        }
    }
    let took = start.elapsed();
    let audio = seconds as f64;
    println!(
        "two strips over {seconds} s of audio: {:.1} ms, {:.4}% of realtime",
        took.as_secs_f64() * 1000.0,
        took.as_secs_f64() / audio * 100.0,
    );
    println!(
        "per {BUFFER}-frame buffer: {:.1} us of the {:.1} us it has",
        took.as_secs_f64() * 1e6 / buffers as f64,
        BUFFER as f64 / f64::from(RATE) * 1e6,
    );
}
