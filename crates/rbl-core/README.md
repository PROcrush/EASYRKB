# rbl-core

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Shared identifiers, timestamps, musical-key notation, XML helpers, and durable file publication. This is the common vocabulary for the other crates, not an application service.

## Start here

Start with `ContentId`, `PlaylistId`, `RowIndex`, and `FourCc` in [`src/lib.rs`](src/lib.rs). Database IDs identify persistent records; a row index is valid only within one loaded snapshot.

Read [`tests/primitives.rs`](tests/primitives.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`ids.rs`](src/ids.rs) | ID helpers. |
| [`time.rs`](src/time.rs) | Rekordbox date/time formatting. |
| [`musickey.rs`](src/musickey.rs) | Key notation. |
| [`xml.rs`](src/xml.rs) | Shared XML helpers. |
| [`durable.rs`](src/durable.rs) | Durable filesystem publication. |
| [`paths.rs`](src/paths.rs) | Path helpers. |

## Contracts and safety

Do not add Tauri or network behavior here. Keep filesystem durability guarantees explicit; a successful write and a durably published file are different contracts.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-core
cargo clippy -p rbl-core --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

