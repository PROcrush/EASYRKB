# Waveform scrubbing

[Documentation](../README.md)

Drag a waveform to play audio at the drag's speed and direction. Slow drags
sound darker; faster drags expose more high frequencies. The filter applies
to every scrub, regardless of whether the audio contains a kick.

## Speed and tone

Cutoff follows a concave quadratic ease-out curve from 400 Hz at rest to
19 kHz at normal playback speed, staying at 19 kHz above 1×. It opens quickly
at low speeds and approaches the maximum smoothly. Speed is audio traversed
per second, so waveform zoom changes how much audio a mouse movement crosses.
Forward and reverse use the same curve.

A stereo two-pole Butterworth low-pass follows the smoothed playback rate.
Coefficients glide with a 5 ms time constant. Cutoff is capped at 40% of the
output device sample rate. The stop fade follows the filter to keep a
stationary hand silent. Releasing the drag discards the scrub filter and
returns playback to its normal audio path.

## Code and validation

The implementation is `crates/rbl-deck/src/scrub.rs`; `deck.rs` supplies the
output sample rate. Tests cover response at 44.1, 48, and 96 kHz, direction
symmetry, stereo isolation, smoothing, low-rate stability, motion, and stopping.

```sh
RB_LITE_TEST=1 cargo test -p rbl-deck
```

These are DSP tests, not a physical controller check. See [Testing](../development/testing.md)
for the distinction between audio, interface, and hardware evidence.
