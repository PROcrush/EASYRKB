# rbl-audio

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Decodes audio into mono floating-point samples for analysis and provides export-format compatibility handling. Playback streaming belongs to `rbl-deck`.

## Start here

Start with `Audio` and `decode_mono(path, max_secs)` in [`src/lib.rs`](src/lib.rs); the caller supplies the decode duration limit.

Read [`src/lib.rs`](src/lib.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | Symphonia decoding and mono sample buffers. |
| [`compatibility.rs`](src/compatibility.rs) | Audio compatibility checks and conversion. |

## Contracts and safety

Bound decoding work for long files. Keep source sample rate and channel information distinct from the mono analysis buffer. Add fixtures for codec, rate, channel, and truncated-input changes; avoid using a developer's music collection as a test dependency.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-audio
cargo clippy -p rbl-audio --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

