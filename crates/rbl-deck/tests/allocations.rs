//! Nothing allocates inside the audio callback.
//!
//! An allocation on the audio thread is a lock on a global structure taken in
//! the one place that cannot afford to wait: it is bounded by nothing, and
//! when it does block, what comes out of the device is a click. The plan lists
//! "0 callback allocations" as a release budget, so this counts them.
//!
//! The counter is thread-local and only armed around the render call, so the
//! decode threads' own allocations — which are fine, they are not realtime —
//! are not counted, and neither is the harness that drives it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
// The one place unsafe is allowed in this workspace, and it is not shipped:
// counting allocations means being the allocator, and `GlobalAlloc` is an
// unsafe trait. Nothing here is compiled into the application.
#![allow(unsafe_code, reason = "a counting allocator is the only way to count allocations")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use rbl_deck::{Deck, DeckEvent, Engine, Render, Sink};

const RATE: u32 = 44_100;
const BUFFER: usize = 512;

thread_local! {
    /// Allocations on this thread while the watch is armed.
    static COUNT: Cell<usize> = const { Cell::new(0) };
    /// Whether they are being counted. Off by default, so every thread that
    /// is not deliberately watching pays one branch and nothing else.
    static WATCHING: Cell<bool> = const { Cell::new(false) };
}

struct Counting;

// The counter is a `Cell` in thread-local storage with a const initialiser, so
// reading or writing it cannot itself allocate and cannot recurse into here.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        note();
        unsafe { System.realloc(ptr, layout, new) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc_zeroed(layout) }
    }
}

fn note() {
    let _ = WATCHING.try_with(|watching| {
        if watching.get() {
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        }
    });
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// A sink that owns its buffer, so pulling it allocates nothing of its own.
///
/// `NullSink` builds a `Vec` a pull, which is the right shape for a test that
/// wants the samples but the wrong one for a test that counts allocations.
struct Watched {
    render: Mutex<Render>,
    buffer: Mutex<Vec<f32>>,
    running: Mutex<bool>,
}

impl Watched {
    fn new(render: Render) -> Self {
        Self {
            render: Mutex::new(render),
            buffer: Mutex::new(vec![0.0; BUFFER * 2]),
            running: Mutex::new(false),
        }
    }

    /// One callback's worth, counting what it allocated.
    fn pull(&self, count_it: bool) -> usize {
        let mut buffer = self.buffer.lock().unwrap();
        let mut render = self.render.lock().unwrap();
        buffer.fill(0.0);
        COUNT.with(|c| c.set(0));
        WATCHING.with(|w| w.set(count_it));
        render(&mut buffer);
        WATCHING.with(|w| w.set(false));
        COUNT.with(Cell::get)
    }
}

impl Sink for Watched {
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn start(&self) -> rbl_deck::Result<()> {
        *self.running.lock().unwrap() = true;
        Ok(())
    }
    fn stop(&self) -> rbl_deck::Result<()> {
        *self.running.lock().unwrap() = false;
        Ok(())
    }
}

/// A ramp, so there is something real to decode.
fn wav(path: &Path, frames: usize) {
    let mut out = Vec::new();
    let data = u32::try_from(frames * 2 * 2).unwrap();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 4).to_le_bytes());
    out.extend_from_slice(&4_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for frame in 0..frames {
        let value = (0.5 + 0.5 * (frame as f32 / frames as f32)) * 32_767.0;
        let sample = (value as i16).to_le_bytes();
        out.extend_from_slice(&sample);
        out.extend_from_slice(&sample);
    }
    std::fs::write(path, out).unwrap();
}

#[test]
fn the_audio_callback_allocates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp.wav");
    wav(&path, RATE as usize * 4);

    let (tx, rx) = mpsc::channel();
    let events: rbl_deck::EventSink = Arc::new(move |event: DeckEvent| {
        let _ = tx.send(event);
    });
    let (sink_tx, sink_rx) = mpsc::channel();
    let engine = Engine::with_sink(
        move |render| {
            let sink = Arc::new(Watched::new(render));
            let _ = sink_tx.send(Arc::clone(&sink));
            Ok(sink as Arc<dyn Sink>)
        },
        &events,
    )
    .expect("engine");
    let sink = sink_rx.recv().expect("sink");

    engine.load(Deck::A, &path);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !engine.snapshot().a.loaded && Instant::now() < deadline {
        let _ = rx.try_recv();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(engine.snapshot().a.loaded, "the deck never loaded");
    engine.play(Deck::A);

    // The first callback builds what it needs — the channel strips, the
    // smoothing — and that is allowed: it happens once, before any of this is
    // realtime. Warmed with the counter off.
    for _ in 0..20 {
        sink.pull(false);
        std::thread::sleep(Duration::from_millis(1));
    }

    // And then it must never allocate again, whatever the transport does.
    let mut worst = 0;
    for i in 0..200 {
        match i {
            40 => engine.seek_frames(Deck::A, u64::from(RATE) * 2),
            80 => engine.pause(Deck::A),
            100 => engine.play(Deck::A),
            140 => engine.master().set_gain(0.3),
            160 => engine.mixer().set_crossfade(0.8),
            // The tempo control puts a stretcher in the path, which is the
            // most likely place for a buffer to be grown mid-track.
            170 => engine.set_tempo(Deck::A, 1.06),
            180 => engine.set_master_tempo(Deck::A, true),
            190 => engine.set_tempo(Deck::A, 0.94),
            _ => {}
        }
        worst = worst.max(sink.pull(true));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(worst, 0, "the callback allocated {worst} times in a buffer");
}
