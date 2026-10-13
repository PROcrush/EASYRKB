# rbl-fakecdj

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

A protocol-client stand-in for a player. It exercises RemoteDB and NFS servers over sockets without booting player firmware.

## Start here

Read `Database`, `database_port`, `mount`, and `Mounted` in [`src/lib.rs`](src/lib.rs), then the loopback test for an end-to-end example.

Read [`tests/loopback.rs`](tests/loopback.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | RemoteDB client, UDP RPC client, mount, lookup, and file reads. |

## Contracts and safety

Default to isolated loopback servers and ephemeral ports. This client does not reproduce a player's UI or firmware and must not be reported as hardware emulation. Contacting real rekordbox or a player is a deliberate read-only diagnostic, not a normal unit test.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-fakecdj
cargo clippy -p rbl-fakecdj --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

