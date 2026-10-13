# rbl-link

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Orchestrates RBXport as a Pro DJ Link library source: network join, announcements, database browsing, analysis replies, NFS file delivery, and player status.

## Start here

Read `LinkExport::start`, `Interface`, and `Ports` in [`src/lib.rs`](src/lib.rs). Follow `IndexCatalog` to `rbl-dbserver`, and file delivery to `rbl-nfs`. Use `Ports::EPHEMERAL` on loopback for tests.

Read [`tests/link.rs`](tests/link.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`join.rs`](src/join.rs) | Device-number negotiation. |
| [`beacon.rs`](src/beacon.rs) | Announcements and player state. |
| [`catalog.rs`](src/catalog.rs) | Index-backed browse queries and edits. |
| [`blobs.rs`](src/blobs.rs) | Analysis reply construction. |
| [`files.rs`](src/files.rs) | Exported filesystem. |
| [`watch.rs`](src/watch.rs) | Interface watching. |

## Contracts and safety

Do not run integration tests on a live DJ network by default. Preserve the join gate before announcing or serving. Unit and socket tests establish protocol behavior, not booted-firmware or physical-device results. Retain [OBS], [ASSUME], and [UNKNOWN] distinctions when changing compatibility behavior.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-link
cargo clippy -p rbl-link --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

