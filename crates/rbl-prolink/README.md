# rbl-prolink

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Encodes and decodes Pro DJ Link packets and represents device announcements/status. Socket orchestration belongs to `rbl-link`.

## Start here

Start with packet constants, `AnnounceKind`, and `DeviceType` in [`src/lib.rs`](src/lib.rs); follow the relevant packet encoder/decoder and its test.

Read [`tests/packets.rs`](tests/packets.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | Packet layouts, validation, and device types. |

## Contracts and safety

This crate does not open sockets. Test exact bytes as well as decoded values and malformed lengths. Keep capture-backed observations separate from assumed device behavior; codec round trips alone cannot verify a packet against hardware.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-prolink
cargo clippy -p rbl-prolink --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

