# Playback testing and change workflow

[Playback guide](../README.md) · [Project testing](../../../docs/development/testing.md)

Run these commands from the repository root. Public tests use generated audio
or the committed generated MP3 fixture and do not require AlphaTheta Emulator.

## Choose a test boundary

| Location | Coverage |
| --- | --- |
| `tests/engine.rs` | Engine transport, seeks, timing, silence, load races, two-deck mixing, and output through a test sink. |
| `tests/allocations.rs` | No callback allocations after initialization under the exercised transport operations. |
| `tests/rubberband.rs` | Default-feature stretch/key behavior. |
| Module unit tests | Decoder alignment, scrub/filter DSP, smoothing, metering, and other local contracts. |
| `tests/fixtures/` | Generated MP3 used for PCM timeline comparison after seeks. |

## Run checks

```sh
RB_LITE_TEST=1 cargo test -p rbl-deck
RB_LITE_TEST=1 cargo test -p rbl-deck --test engine
RB_LITE_TEST=1 cargo test -p rbl-deck --test allocations
RB_LITE_TEST=1 cargo test -p rbl-deck mp3_seeks_keep_the_sequential_audio_timeline
cargo clippy -p rbl-deck --all-targets -- -D warnings
```

When changing feature-dependent code, also check the fallback build:

```sh
cargo check -p rbl-deck --no-default-features
```

The [fixture guide](../tests/fixtures/README.md) describes optional FFmpeg
regeneration. Ordinary tests consume the committed fixture and do not need
FFmpeg installed.

## Make a playback change

Use a generated signal whose expected output is measurable, then choose
whether to check decoder PCM, a DSP stage, or engine output. A correct position
counter does not establish correct samples; seek regressions compare actual
audio against the sequential decode timeline.

For callback changes, preserve generation handling, silence on missing audio,
and the allocation/locking boundary. Run engine and allocation coverage
alongside the affected module tests. For scrub/filter changes, cover sample
rate, direction, stereo isolation, smoothing, and stop/release behavior.

Update the guide when transport, output, or feature behavior changes. Follow
[project conventions](../../../docs/development/conventions.md), and report
which of unit DSP, test sink, physical audio device, or firmware was actually
exercised. A test sink does not validate a platform driver's behavior.
