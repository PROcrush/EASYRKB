# Rust crate guide

[Project documentation](../docs/README.md) · [Architecture](../docs/development/architecture.md)

Start with the crate that owns the behavior you want to change. Each README
provides a reading path, code map, safety contracts, and focused checks.
These crates are independent of Tauri; application coordination belongs in
`src-tauri/`, and frontend access goes through `src/ipc/`.

## Library and media

| Crate | Responsibility |
| --- | --- |
| [rbl-core](rbl-core/README.md) | Shared primitives and durable publication. |
| [rbl-db](rbl-db/README.md) | Master library access, guarded writes, and fixtures. |
| [rbl-index](rbl-index/README.md) | In-memory browsing, sorting, filtering, and searching. |
| [rbl-anlz](rbl-anlz/README.md) | Analysis-file parsing and authoring. |
| [rbl-audio](rbl-audio/README.md) | Audio decoding and export compatibility. |
| [rbl-analysis](rbl-analysis/README.md) | Offline tempo, grid, key, and waveform DSP. |
| [rbl-deck](rbl-deck/README.md) | Two-deck playback and real-time audio. |
| [rbl-backup](rbl-backup/README.md) | Backup archives and recoverable restore. |
| [rbl-devices](rbl-devices/README.md) | Volume discovery, inspection, and eject. |

## USB export

| Crate | Responsibility |
| --- | --- |
| [rbl-export](rbl-export/README.md) | Export and synchronization orchestration. |
| [rbl-pdb](rbl-pdb/README.md) | Legacy DeviceSQL binary database. |
| [rbl-onelibrary](rbl-onelibrary/README.md) | Device Library Plus / OneLibrary database. |

## Link Export and diagnostics

| Crate | Responsibility |
| --- | --- |
| [rbl-link](rbl-link/README.md) | Library-source orchestration and player state. |
| [rbl-prolink](rbl-prolink/README.md) | Announcement and status packet codecs. |
| [rbl-dbserver](rbl-dbserver/README.md) | RemoteDB browsing and session replies. |
| [rbl-nfs](rbl-nfs/README.md) | Read-only audio file delivery. |
| [rbl-fakecdj](rbl-fakecdj/README.md) | Socket-level test client, not firmware emulation. |
| [rbl-difftool](rbl-difftool/README.md) | Read-only database change recording. |

## First change

1. Read the owning crate's README, public API, and a nearby test.
2. For database writes, use `RB_LITE_TEST=1` and a temporary
   `rbl_db::fixture::build` library. Never use the installed library as a write fixture.
3. Run the crate's focused tests and lint before broader
   [repository checks](../docs/development/testing.md).
4. Keep format/protocol evidence explicit. Unit tests, socket integration,
   booted firmware, filesystem validation, and physical hardware establish
   different properties.

