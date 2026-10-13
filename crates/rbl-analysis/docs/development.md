# API, code map, and change workflow

[Analysis documentation](README.md) · [Overview](overview.md)

The crate owns signal analysis. Decoding, queues, library guards, and
publication belong to its callers. Begin at `src/lib.rs::analyse_with` to
trace the complete result, then follow the stage you need.

## Input and output contract

| Item | Contract |
| --- | --- |
| Input | Mono `f32` samples and the corresponding sample rate. `rbl-audio` supplies decoded input to the app. |
| Options | `AnalysisOptions` contains tempo, attack, and key options. App preset selection and manual range/placement overrides happen outside stage internals. |
| Tempo | `TempoResult` contains BPM, segments, first-beat time, and the explicit beat list. |
| Beats | `Beat::time_ms` is milliseconds, `tempo_x100` is BPM × 100, and `beat_number` is 1–4. |
| Key | `Option<MusicalKey>`; do not invent a key when detection returns none. |
| Waveform | Detail columns and a separate three-band overview, described in [Waveforms](algorithms/waveform.md). |
| Level | Whole-track peak magnitude and RMS. |

Envelope timestamps and grid seconds are intermediate coordinates, not
interchangeable with samples or ANLZ milliseconds. Preserve origin offsets
when mapping between them. `tempo::beats_of` reconstructs numbered beats from
segments; a segment change must retain the count across its boundary.

## Code map

| Module | Responsibility |
| --- | --- |
| `lib.rs` | Public API, stage coordination, final numbering, file-start adjustment, and result assembly. |
| `onset.rs` | Full-band and band-limited onset envelopes. |
| `attack.rs` | Attack maps used for precise timing. |
| `tempo.rs` | Candidate selection, fits, segmentation, transitions, gap handling, and beat generation. |
| `downbeat.rs` | Band frames, structural novelty, bar phase, and phrase starts. |
| `key.rs` | Spectral/profile front end, naming, optional grid-dependent rules. |
| `waveform.rs` | Detail/overview computation and supported preview packing helpers. |
| `phrase.rs`, `vocal.rs` | Explicit unimplemented interfaces for labels and vocal detection. |

`analyse_with` launches independent audio passes with scoped threads, then
resolves their outputs into a tempo grid. It corrects phase/numbering and
applies file-start adjustment before any grid-dependent key rules run.
The [pipeline](pipeline.md) explains this dependency order; its diagram is a
conceptual flow, not a claim that all passes execute serially.

## Application boundary

The app's typed IPC request reaches `src-tauri/src/analysis.rs`, which validates
requested stages, checks locks, decodes audio, and writes results. Key-only
analysis is an app path that preserves grids and waveform files; the full
crate `analyse_with` call itself computes all stages.

The crate does not author complete ANLZ files or update `master.db`. Those
operations use `rbl-anlz` and `rbl-db`. Preserve existing cues and unchecked
results in integration code; see [Analysis settings](../../../docs/user/analysis-settings.md).

## Make a change

1. Reproduce it with generated audio or a precise intermediate-stage input. Identify whether the error is timing, bar numbering, key, or waveform output.
2. Read the relevant stage reference and [assumptions](assumptions.md). State any new rule generally rather than naming a song in the implementation.
3. Add a regression that checks the desired output: timestamps, beat numbers, preserved segments, or signal characteristics.
4. Run the focused test, then the public crate suite and Clippy. Use private evaluation when available to detect regressions across real tracks.
5. Update algorithm/options documentation and report historical versus newly measured evidence separately.

Follow [project Rust conventions](../../../docs/development/conventions.md#rust).
Do not alter decoding timelines, silence behavior, or numerical thresholds
without documenting the affected contract and checking the relevant regressions.
