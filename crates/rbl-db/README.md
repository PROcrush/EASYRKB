# rbl-db

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Opens the rekordbox SQLCipher master library, probes its schema, and provides library reads, edits, imports, and temporary fixture creation.

## Start here

Follow `LibraryLocation`, `OpenMode`, and `Library::open` in [`src/lib.rs`](src/lib.rs), then the operation-specific module. `rbl-index` loads its browse snapshot from this crate.

Read [`tests/writes.rs`](tests/writes.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | Location detection, connection opening, and write guards. |
| [`schema.rs`](src/schema.rs) | Schema support probing. |
| [`write.rs`](src/write.rs) | Library edits. |
| [`fixture.rs`](src/fixture.rs) | Temporary test library. |
| [`details.rs`](src/details.rs) | Track detail queries. |
| [`import.rs`](src/import.rs) | Import operations. |
| [`new_library.rs`](src/new_library.rs) | New library creation. |
| [`xml.rs`](src/xml.rs) | Rekordbox XML. |
| [`itunes.rs`](src/itunes.rs) | Music-library input. |
| [`export_info.rs`](src/export_info.rs) | Export bookkeeping. |

## Contracts and safety

Opening an installed library is real data access. Never use it for write tests: set `RB_LITE_TEST=1` and use `fixture::build` in a temporary directory. Read-write access is explicit and normally refused while rekordbox is running. Do not bypass those guards or document key material.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-db
cargo clippy -p rbl-db --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

