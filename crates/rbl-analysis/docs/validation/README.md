# Analysis testing and evaluation

[Analysis documentation](../README.md) · [Project testing](../../../../docs/development/testing.md)

Public synthetic tests verify outputs for generated inputs. Private reference
playlists compare results with identified rekordbox/hand grids. Neither is a
physical-player compatibility result. Run commands from the repository root.

## Public tests

```sh
RB_LITE_TEST=1 cargo test -p rbl-analysis
cargo clippy -p rbl-analysis --all-targets -- -D warnings
```

| Location | Coverage |
| --- | --- |
| `tests/analysis.rs` | Tempo and click placement, numbering, key/rule behavior, waveform bands, level, silence, and file-start handling. |
| `tests/multi_tempo_ticks.rs` | Sample-defined markers across cuts and ramps. |
| Unit tests in `src/tempo.rs` | Transition emphasis/release, quiet kickless walks and cuts, rhythm aliases, and gap behavior. |
| Unit tests in `src/waveform.rs` | Silence/impulses, peak reduction, preview counts, and bit layouts. |

For a timing fix, keep both the predicted timestamps and 1–4 numbering under
test. For a transient fix, check preservation of reliable kick timing as well
as the quiet-transition case. For a boundary fix, check silent/delayed starts
and the 20 ms opening-cut boundary alongside the accepted case.

## Private reference evaluation

[Reference playlist evaluation](reference-playlist.md) uses the read-only
`golden` example to cache audio and compare BPM, downbeat, grid, and key.
It requires the named private playlist and its original files. Cache refresh
and evaluation are separate actions; record which source grids the cache holds.

[Multi-tempo evaluation](multitempo.md) compares imported copies in a library.
Its historical write path detects the installed library, so it is not a
suitable writable development test. Use its read-only cache/score alternative
or public generated regressions.

Additional fixture-generation notes live in the private companion repository
at `crates/rbl-analysis/docs/grid-fixtures.md`. They are optional internal
material, not a prerequisite for the public crate tests.

## Report results

State the revision, options, input/reference set, command, and metric results.
Distinguish a synthetic regression from a playlist score. Existing reference
scores predate the transient fallback and have not been remeasured for it;
do not report them as a fresh pass for the current checkout.

The 99% playlist target is a goal, not an assertion that the recorded grid/key
metrics met it. Waveform calibration uses nine calibration tracks and is not
a held-out accuracy result. Hardware/export integration needs its own tests,
listed in the [project test guide](../../../../docs/development/testing.md).
