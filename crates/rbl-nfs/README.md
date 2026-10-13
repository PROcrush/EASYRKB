# rbl-nfs

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Implements the read-only NFSv2 file service used for Link Export, including SUN-RPC, XDR, portmap, mount, and virtual filesystem access.

## Start here

Read `Server::handle` and `Exports`/`Vfs` in [`src/lib.rs`](src/lib.rs) and `src/vfs.rs`. The request handler can be tested with datagrams without binding real network ports.

Read [`tests/protocol.rs`](tests/protocol.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`rpc.rs`](src/rpc.rs) | RPC envelope and authentication framing. |
| [`xdr.rs`](src/xdr.rs) | Binary field readers/writers. |
| [`vfs.rs`](src/vfs.rs) | Exports, handles, and attributes. |
| [`lib.rs`](src/lib.rs) | RPC program dispatch and file procedures. |
| [`net.rs`](src/net.rs) | Socket serving. |

## Contracts and safety

rekordbox uses portmap 50111 and UTF-16LE names; do not replace these with generic NFS assumptions. Mutating procedures must remain read-only refusals. Test path traversal, handles, read bounds, EOF, and malformed packets against temporary files.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-nfs
cargo clippy -p rbl-nfs --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

