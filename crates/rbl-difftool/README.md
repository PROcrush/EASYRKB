# rbl-difftool

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Records database snapshots and diffs to discover what rekordbox changes after one controlled action. It is a developer diagnostic tool, not a library editor.

## Start here

Read `snapshot`, `diff`, and `Snapshot` in [`src/lib.rs`](src/lib.rs); `src/main.rs` implements the CLI. Small tables retain full rows; large tables retain per-row digests.

Read [`src/lib.rs`](src/lib.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | Snapshot representation, diffing, and JSON persistence. |
| [`main.rs`](src/main.rs) | snapshot, diff, inspect, and guided record commands. |

## Contracts and safety

The database is read-only, but snapshots may contain private library metadata. Keep captures out of public commits. For a meaningful comparison, quit rekordbox, capture before, perform exactly one action in rekordbox, quit it, and capture after. Tests must use temporary databases.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-difftool
cargo clippy -p rbl-difftool --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

