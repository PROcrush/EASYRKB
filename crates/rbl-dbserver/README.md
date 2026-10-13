# rbl-dbserver

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Implements the RemoteDB messages and browse sessions a player uses to query a library. `rbl-link` supplies the application catalog.

## Start here

Begin with `Message` and `Argument` in [`src/lib.rs`](src/lib.rs), then `session::CatalogHandler` and the `Catalog` contract. Port 12523 is the discovery/query service, not necessarily the database session port.

Read [`tests/session.rs`](tests/session.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | Wire framing, arguments, and request kinds. |
| [`session.rs`](src/session.rs) | Request dispatch and session behavior. |
| [`catalog.rs`](src/catalog.rs) | Catalog query/edit interface. |
| [`item.rs`](src/item.rs) | Menu item encoding. |
| [`filter.rs`](src/filter.rs) | Filter representation. |
| [`keys.rs`](src/keys.rs) | Key-menu handling. |
| [`net.rs`](src/net.rs) | TCP serving and port discovery. |

## Contracts and safety

Keep message framing independent of catalog semantics. Test headers, items, footers, pagination, empty/error responses, and model-specific request aliases. Captured corpus tests are stronger than self-generated round trips, but neither alone proves every device UI behavior.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-dbserver
cargo clippy -p rbl-dbserver --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

