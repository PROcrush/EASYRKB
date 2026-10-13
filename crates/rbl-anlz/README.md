# rbl-anlz

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Parses, preserves, edits, and authors rekordbox analysis files (`.DAT`, `.EXT`, `.2EX`). It handles format bytes; `rbl-analysis` computes DSP results.

## Start here

Begin with `parse`, `Anlz`, and `Section` in [`src/lib.rs`](src/lib.rs). Sections retain tag-specific header bytes and payload separately so unknown tags can survive a round trip.

Read [`tests/parse.rs`](tests/parse.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | File framing, sections, and decoded accessors. |
| [`write.rs`](src/write.rs) | ANLZ builder. |
| [`encode.rs`](src/encode.rs) | Analysis-file authoring. |
| [`grid.rs`](src/grid.rs) | Beat-grid edits. |
| [`cues.rs`](src/cues.rs) | Cue edits. |
| [`phrase.rs`](src/phrase.rs) | Song-structure representation and edits. |
| [`vocal.rs`](src/vocal.rs) | Vocal data representation. |

## Contracts and safety

Do not interpret tag header fields as payload. Preserve unknown sections and test byte-level round trips. File edits must use disposable fixtures, never installed analysis files. Format support is not proof of DSP or physical-player compatibility.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-anlz
cargo clippy -p rbl-anlz --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

