//! The unit of audio that crosses from a decode thread to the audio callback.
//!
//! A block rather than a stream of samples, because a block can say which
//! generation it belongs to and where in the track it starts. That is what
//! makes a seek exact: the callback drops every block decoded before the seek
//! and takes the playhead from the first one after it, rather than trying to
//! reason about how much stale audio is still in flight.
//!
//! Plain data, no heap: moving one into the ring is a copy of about four
//! kilobytes, and dropping one inside the audio callback frees nothing.

/// Frames in a full block: about 11.6 ms at 44.1 kHz.
pub const BLOCK_FRAMES: usize = 512;

/// Blocks a deck's ring holds — about 186 ms, which absorbs a decode hiccup
/// without the callback running dry.
pub const RING_BLOCKS: usize = 16;

#[derive(Clone, Copy)]
pub struct Block {
    /// The generation this was decoded under.
    pub generation: u32,
    /// Where its first frame sits in the track, in device-rate frames.
    pub position: u64,
    /// How many of `samples` are real. The last block of a track is short.
    pub frames: u16,
    /// Stereo, interleaved, at the device rate.
    pub samples: [f32; BLOCK_FRAMES * 2],
}

impl Block {
    pub fn empty(generation: u32, position: u64) -> Self {
        Self { generation, position, frames: 0, samples: [0.0; BLOCK_FRAMES * 2] }
    }

    /// The interleaved samples that are real.
    pub fn filled(&self) -> &[f32] {
        let end = (self.frames as usize * 2).min(self.samples.len());
        self.samples.get(..end).unwrap_or(&[])
    }
}

#[allow(clippy::missing_fields_in_debug, reason = "the samples are the point of not deriving this")]
impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Without this the derive would print 1,024 floats.
        f.debug_struct("Block")
            .field("generation", &self.generation)
            .field("position", &self.position)
            .field("frames", &self.frames)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_block_reports_only_what_it_holds() {
        let mut block = Block::empty(1, 0);
        block.frames = 3;
        assert_eq!(block.filled().len(), 6);
    }

    #[test]
    fn a_frame_count_past_the_buffer_cannot_read_past_it() {
        let mut block = Block::empty(1, 0);
        block.frames = u16::MAX;
        assert_eq!(block.filled().len(), BLOCK_FRAMES * 2);
    }
}
