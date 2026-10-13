//! What two decks stretching at once cost, as a share of the time they have.
//!
//! The plan's first task for the tempo work: measure the CPU spike before
//! deciding anything else about it. Two stretchers, run over a minute of audio
//! each at a realistic tempo offset, timed against realtime — both backends,
//! because Rubber Band R3 buys pitch accuracy with arithmetic and performance
//! is principle #1. Two decks at once is the case that has to fit.
//!
//! `cargo run --release -p rbl-deck --example stretchcost`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use rbl_deck::{Stretcher, Wsola};

const RATE: u32 = 44_100;
const BUFFER: usize = 512;

fn main() {
    let seconds = 60_usize;
    // Something with content at every frequency, so the similarity search has
    // real work to do rather than locking onto one period.
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut noise = |n: usize| -> Vec<f32> {
        (0..n * 2)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 40) as f32 / 8_388_608.0 - 1.0
            })
            .collect()
    };

    let buffers = seconds * RATE as usize / BUFFER;
    let feed = noise(BUFFER * 2);

    for ratio in [1.0_f32, 1.06, 0.92] {
        run(
            "WSOLA     ",
            ratio,
            seconds,
            buffers,
            &feed,
            [Box::new(Wsola::new(RATE)) as Box<dyn Stretcher>, Box::new(Wsola::new(RATE))],
        );
        #[cfg(feature = "rubberband")]
        {
            let Some(a) = rbl_deck::RubberBand::new(RATE) else { continue };
            let Some(b) = rbl_deck::RubberBand::new(RATE) else { continue };
            run("Rubber Band", ratio, seconds, buffers, &feed, [Box::new(a), Box::new(b)]);
        }
    }
}

fn run(
    label: &str,
    ratio: f32,
    seconds: usize,
    buffers: usize,
    feed: &[f32],
    mut decks: [Box<dyn Stretcher>; 2],
) {
    for deck in &mut decks {
        deck.set_ratio(ratio);
    }
    let mut out = vec![0.0_f32; BUFFER * 2];
    let start = Instant::now();
    let mut short = 0_usize;
    let mut produced = 0_usize;
    for _ in 0..buffers {
        for deck in &mut decks {
            // Feed, take what came of it, feed again, the way the deck does.
            // Rubber Band asks for a block of its own choosing and then wants
            // nothing more until it has been drained, so one feed then one
            // pull fills exactly half a buffer — the older loop timed that and
            // called it fast.
            let mut got = 0;
            while got < BUFFER {
                while !deck.ready(BUFFER - got) && deck.wants() > 0 {
                    deck.feed(feed);
                }
                let more = deck.pull(&mut out[got * 2..]);
                if more == 0 {
                    break;
                }
                got += more;
            }
            produced += got;
            if got < BUFFER {
                short += 1;
            }
        }
    }
    let took = start.elapsed();
    // A run that could not fill its buffers measured an idle loop, not a
    // stretcher, so the count is printed rather than trusted.
    println!(
        "{label} two stretchers at {ratio:>5}x over {seconds} s: {:>7.1} ms, {:>6.3}% of \
         realtime, {:>6.1} us per {BUFFER}-frame buffer of the {:.0} it has",
        took.as_secs_f64() * 1000.0,
        took.as_secs_f64() / seconds as f64 * 100.0,
        took.as_secs_f64() * 1e6 / buffers as f64,
        BUFFER as f64 / f64::from(RATE) * 1e6,
    );
    // What matters is whether it kept up, not whether every pull was full: a
    // deck takes a short block, it does not glitch on one.
    let wanted = buffers * BUFFER * 2;
    println!(
        "    (produced {:.1}% of the audio asked for; {short} of {} pulls came up short)",
        produced as f64 / wanted as f64 * 100.0,
        buffers * 2,
    );
}
