# Playback audio fixtures

[Playback guide](../../README.md) · [Testing](../../docs/testing.md)

`seek-noise.mp3` is generated pink noise, not a music excerpt. It provides a
repeatable compressed signal for MP3 seek alignment tests. It is committed;
running tests does not require regenerating it or installing FFmpeg.

## Consumer and expected behavior

`src/decode.rs::mp3_seeks_keep_the_sequential_audio_timeline` copies the fixture
into a temporary directory. It compares PCM after forward, backward, and zero
seeks with a sequential decode of the same file, excluding decoder warmup.
The comparison catches a one-packet offset that a cursor-only assertion misses.

## Regenerate

Run this from `crates/rbl-deck/tests/fixtures/` with FFmpeg's LAME encoder:

```sh
ffmpeg -v error -f lavfi \
  -i 'anoisesrc=color=pink:sample_rate=44100:duration=2:seed=42:amplitude=0.3' \
  -ac 2 -c:a libmp3lame -b:a 64k -y seek-noise.mp3
```

The fixture is two seconds of stereo audio at 44.1 kHz, encoded at 64 kbps.
The command replaces the fixture file in this directory. Run the focused
regression after regeneration:

```sh
# From the repository root:
RB_LITE_TEST=1 cargo test -p rbl-deck mp3_seeks_keep_the_sequential_audio_timeline
```

If changing the encoding changes decoder warmup or alignment, explain the
reason in the patch and keep the PCM comparison meaningful. Do not replace
generated fixtures with private music.
