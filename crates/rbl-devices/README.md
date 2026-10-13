# rbl-devices

[Crate index](../README.md) · [Architecture](../../docs/development/architecture.md)

Enumerates mounted volumes, inspects existing USB exports, watches mount changes, and supports device settings and eject operations.

## Start here

Read `Device`, `DeviceExport`, `list`, and `inspect` in [`src/lib.rs`](src/lib.rs). Enumeration uses OS volume information; inspection performs device I/O and should run on demand.

Read [`tests/inspect.rs`](tests/inspect.rs) for existing cases and expected behavior.
The [manifest](Cargo.toml) lists dependencies and feature flags.

## Code map

| File in `src/` | Responsibility |
| --- | --- |
| [`lib.rs`](src/lib.rs) | Volume enumeration and export inspection. |
| [`mounts.rs`](src/mounts.rs) | MountWatcher. |
| [`settings.rs`](src/settings.rs) | Device settings. |
| [`explorer.rs`](src/explorer.rs) | Device browsing. |
| [`eject.rs`](src/eject.rs) | Platform eject operations. |
| [`format.rs`](src/format.rs) | Erases a whole stick and formats it as FAT32 or HFS+ (macOS). |

## Contracts and safety

Do not repeatedly inspect every USB device in a polling loop. Use `RB_LITE_FAKE_VOLUMES` with temporary directories for tests rather than real removable media. Eject and device writes require an explicitly selected target.

## Run focused checks

From the repository root:

```sh
RB_LITE_TEST=1 cargo test -p rbl-devices
cargo clippy -p rbl-devices --all-targets -- -D warnings
```

Follow the [test guide](../../docs/development/testing.md) for broader checks
and the [contribution guide](../../CONTRIBUTING.md) before preparing a change.

