# rbl-backup

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Shared backup archive, summary, and restore logic for RBXport and its restore application. Archives carry the master database, analysis, artwork, and selected library-side files.

## Start here

Read the archive layout in [`src/lib.rs`](src/lib.rs), then `manifest`, `summary`, and `restore::Request`. Follow the restore journal before changing publication or recovery behavior.

Read [`src/restore.rs`](src/restore.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`archive.rs`](src/archive.rs) | ZIP reading, classification, and copying. |
| [`manifest.rs`](src/manifest.rs) | Archive identity and selected parts. |
| [`summary.rs`](src/summary.rs) | Counts and sizes. |
| [`restore.rs`](src/restore.rs) | Restore phases and execution. |
| [`journal.rs`](src/journal.rs) | Interrupted-operation recovery. |
| [`sizes.rs`](src/sizes.rs) | Size accounting. |

## Contracts and safety

Restores replace real library data. Test only temporary libraries and archives, with `RB_LITE_TEST=1` and an isolated `RBXPORT_STATE_DIR`. Preserve path validation, allowed-entry handling, space checks, cancellation, and recovery; do not treat restore as ordinary ZIP extraction.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-backup
cargo clippy -p rbl-backup --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

