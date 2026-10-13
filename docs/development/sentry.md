# Sentry crash reporting

[Documentation](../README.md) · [Debugging](debugging.md)

Crash reporting is optional for local builds. The renderer uses the build-time
`VITE_SENTRY_DSN`; native Rust uses `SENTRY_DSN`. Each client needs its own DSN.
The renderer skips initialization in Vite development mode.

## Reported data

The app reports unhandled renderer errors, native panics, and internal backend
errors. Backend error messages are omitted because they can contain library
paths or track metadata. User/request context and breadcrumbs are removed;
performance tracing and session replay are not enabled.

The implementations are `src/lib/sentry.ts` and `src-tauri/src/sentry.rs`.
Check them when changing telemetry; do not broaden collection incidentally.

## Configure a packaged build

Supply `VITE_SENTRY_DSN` and `SENTRY_DSN` from an external secrets vault in the
build environment. Source-map uploads also require `SENTRY_AUTH_TOKEN`.
Then build the desktop app:

```sh
pnpm tauri build
```

Public contributor builds do not require these values. The source-map auth
token is used by Vite at build time and must not be included in the frontend
bundle or committed to the repository.

## Source maps

Vite uploads hidden maps for `rbxport@<version>`, then deletes them before
Tauri packages `dist/`. Keep the build version synchronized; see [Releases](releases.md).
