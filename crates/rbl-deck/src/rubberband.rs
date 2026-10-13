//! The Rubber Band backend: the stretcher the trait was written to accept.
//!
//! Rubber Band R3, compiled from the vendored sources by `build.rs`, in its
//! real-time mode. It is here because the WSOLA backend beside it cannot hold
//! a pitch: asked for two semitones it came back a quarter of a semitone out,
//! which is a key shift that puts a track in the wrong key. R3 shifts pitch
//! itself rather than by the resample-and-stretch-back arrangement, so the
//! error that made KEY SYNC unshippable does not arise.
//!
//! **The licence is the cost.** Rubber Band is GPL-or-commercial, so a build
//! with the `rubberband` feature on is distributable under the GPL and nothing
//! else. That is what the `Stretcher` trait was for: the feature turns this
//! file off and [`Wsola`](crate::Wsola) takes over, tempo intact and no key
//! shift offered.
//!
//! Every call into the library happens in the audio thread, so nothing here
//! allocates after `new`: the scratch buffers are sized once, and a block
//! bigger than they hold is fed in pieces rather than growing them.

#![allow(unsafe_code)] // A C library has a C interface; this is the whole of it.

use crate::stretch::{Stretcher, MAX_RATIO, MIN_RATIO};

mod ffi {
    #![allow(non_camel_case_types)]
    use std::ffi::c_void;

    pub type RubberBandState = *mut c_void;

    /// Process blocks as they arrive rather than studying the whole file.
    pub const OPTION_PROCESS_REALTIME: i32 = 0x0000_0001;
    /// R3, the "finer" engine. R2 is the older one and is not what this is for.
    pub const OPTION_ENGINE_FINER: i32 = 0x2000_0000;

    unsafe extern "C" {
        pub fn rubberband_new(
            sample_rate: u32,
            channels: u32,
            options: i32,
            initial_time_ratio: f64,
            initial_pitch_scale: f64,
        ) -> RubberBandState;
        pub fn rubberband_delete(state: RubberBandState);
        pub fn rubberband_reset(state: RubberBandState);
        pub fn rubberband_set_time_ratio(state: RubberBandState, ratio: f64);
        pub fn rubberband_set_pitch_scale(state: RubberBandState, scale: f64);
        pub fn rubberband_set_max_process_size(state: RubberBandState, samples: u32);
        pub fn rubberband_get_samples_required(state: RubberBandState) -> u32;
        pub fn rubberband_process(
            state: RubberBandState,
            input: *const *const f32,
            samples: u32,
            final_: i32,
        );
        pub fn rubberband_available(state: RubberBandState) -> i32;
        pub fn rubberband_retrieve(
            state: RubberBandState,
            output: *const *mut f32,
            samples: u32,
        ) -> u32;
    }
}

/// The largest block handed to the library in one call.
///
/// Everything is sized from it, and `feed` splits anything longer rather than
/// reallocating. 8192 frames is 186 ms at 44.1 kHz — comfortably more than a
/// device callback asks for, at any buffer size a DJ would choose.
const MAX_BLOCK: usize = 8192;

const CHANNELS: usize = 2;

/// Rubber Band R3, behind [`Stretcher`].
pub struct RubberBand {
    state: ffi::RubberBandState,
    /// Deinterleaved input, one contiguous run per channel.
    scratch_in: Vec<f32>,
    /// Deinterleaved output, the same shape.
    scratch_out: Vec<f32>,
    ratio: f32,
    pitch_scale: f32,
}

// The library holds no global state and this owns its instance outright; the
// deck moves it to the audio thread the same way it moves `Wsola`.
unsafe impl Send for RubberBand {}

impl RubberBand {
    /// A stretcher at the file's own speed and pitch.
    ///
    /// Returns `None` if the library refuses to allocate, which is the only
    /// failure it reports — a null state rather than an error code.
    #[must_use]
    pub fn new(sample_rate: u32) -> Option<Self> {
        let options = ffi::OPTION_PROCESS_REALTIME | ffi::OPTION_ENGINE_FINER;
        // SAFETY: the arguments are plain values and the result is checked for
        // null before anything is done with it.
        let state = unsafe {
            ffi::rubberband_new(sample_rate, CHANNELS as u32, options, 1.0, 1.0)
        };
        if state.is_null() {
            return None;
        }
        // SAFETY: `state` is non-null and was made by `rubberband_new`. Telling
        // it the largest block up front is what keeps it from allocating later,
        // in the audio thread.
        unsafe { ffi::rubberband_set_max_process_size(state, MAX_BLOCK as u32) };
        Some(Self {
            state,
            scratch_in: vec![0.0; MAX_BLOCK * CHANNELS],
            scratch_out: vec![0.0; MAX_BLOCK * CHANNELS],
            ratio: 1.0,
            pitch_scale: 1.0,
        })
    }

    /// Feeds one block, at most `MAX_BLOCK` frames, and returns what it took.
    fn feed_block(&mut self, input: &[f32]) -> usize {
        let frames = (input.len() / CHANNELS).min(MAX_BLOCK);
        if frames == 0 {
            return 0;
        }
        let (left, right) = self.scratch_in.split_at_mut(MAX_BLOCK);
        for (i, frame) in input.chunks_exact(CHANNELS).take(frames).enumerate() {
            left[i] = frame[0];
            right[i] = frame[1];
        }
        let planes: [*const f32; CHANNELS] = [left.as_ptr(), right.as_ptr()];
        // SAFETY: two plane pointers for the two channels the state was made
        // with, each valid for `frames` reads because `frames <= MAX_BLOCK` and
        // both halves are that long.
        unsafe { ffi::rubberband_process(self.state, planes.as_ptr(), frames as u32, 0) };
        frames
    }
}

impl Drop for RubberBand {
    fn drop(&mut self) {
        // SAFETY: made by `rubberband_new`, non-null since `new` checked, and
        // dropped exactly once.
        unsafe { ffi::rubberband_delete(self.state) };
    }
}

impl Stretcher for RubberBand {
    fn set_ratio(&mut self, ratio: f32) {
        let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
        self.ratio = ratio;
        // The trait's ratio is how much input one unit of output covers, so
        // 1.06 is six percent fast. Rubber Band's is the length of the output
        // over the length of the input, which is the other way up.
        // SAFETY: a live state and a finite, non-zero ratio.
        unsafe { ffi::rubberband_set_time_ratio(self.state, 1.0 / f64::from(ratio)) };
    }

    fn ratio(&self) -> f32 {
        self.ratio
    }

    fn shifts_pitch(&self) -> bool {
        true
    }

    fn set_pitch_scale(&mut self, scale: f32) {
        // A whole octave either way, which is four times what the semitone
        // buttons offer and past anything a key shift asks for.
        let scale = scale.clamp(0.5, 2.0);
        self.pitch_scale = scale;
        // SAFETY: a live state and a finite, non-zero scale.
        unsafe { ffi::rubberband_set_pitch_scale(self.state, f64::from(scale)) };
    }

    fn pitch_scale(&self) -> f32 {
        self.pitch_scale
    }

    fn wants(&self) -> usize {
        // SAFETY: a live state; the call only reads.
        let required = unsafe { ffi::rubberband_get_samples_required(self.state) } as usize;
        required.min(MAX_BLOCK)
    }

    fn feed(&mut self, input: &[f32]) -> usize {
        // Never more than it asked for. R3 in real-time mode wants each
        // `process` call to carry the block it named, and pushing a whole
        // buffer in regardless made it accept the audio and then produce
        // nothing for it: half the pulls in the cost probe came up short until
        // this was a cap rather than a loop.
        let wanted = self.wants();
        let frames = (input.len() / CHANNELS).min(wanted);
        if frames == 0 {
            return 0;
        }
        self.feed_block(&input[..frames * CHANNELS])
    }

    fn ready(&self, frames: usize) -> bool {
        // SAFETY: a live state; the call only reads.
        let available = unsafe { ffi::rubberband_available(self.state) };
        // -1 means the stream is finished, which for a deck that is still
        // playing means there is nothing more to give.
        available >= 0 && available as usize >= frames
    }

    fn pull(&mut self, out: &mut [f32]) -> usize {
        let mut written = 0;
        while written * CHANNELS < out.len() {
            // SAFETY: a live state; the call only reads.
            let available = unsafe { ffi::rubberband_available(self.state) };
            if available <= 0 {
                break;
            }
            let want = ((out.len() / CHANNELS) - written).min(MAX_BLOCK).min(available as usize);
            if want == 0 {
                break;
            }
            let (left, right) = self.scratch_out.split_at_mut(MAX_BLOCK);
            let planes: [*mut f32; CHANNELS] = [left.as_mut_ptr(), right.as_mut_ptr()];
            // SAFETY: two plane pointers for the two channels the state was
            // made with, each valid for `want` writes because `want` is at
            // most `MAX_BLOCK` and both halves are that long.
            let got = unsafe {
                ffi::rubberband_retrieve(self.state, planes.as_ptr(), want as u32) as usize
            };
            if got == 0 {
                break;
            }
            for i in 0..got {
                let at = (written + i) * CHANNELS;
                out[at] = left[i];
                out[at + 1] = right[i];
            }
            written += got;
        }
        written
    }

    fn reset(&mut self) {
        // SAFETY: a live state; the call only drops what it holds.
        unsafe { ffi::rubberband_reset(self.state) };
    }
}
