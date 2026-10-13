//! Minimal Sentry setup for native crashes.
//!
//! The DSN is baked into release builds through `SENTRY_DSN`; it is a public
//! ingestion identifier, but its source of truth remains 1Password.  No DSN
//! means a deliberately disabled client, keeping local and contributor builds
//! completely offline.

use std::sync::Arc;

/// Installs panic reporting and returns the guard that must outlive the app.
pub fn install() -> sentry::ClientInitGuard {
    let configured_dsn = option_env!("SENTRY_DSN");
    let dsn = configured_dsn.and_then(|value| match value.parse() {
        Ok(dsn) => Some(dsn),
        Err(error) => {
            tracing::warn!(%error, "SENTRY_DSN is invalid; crash reporting is disabled");
            None
        }
    });

    sentry::init(sentry::ClientOptions {
        dsn,
        release: Some(format!("rbxport@{}", env!("CARGO_PKG_VERSION")).into()),
        environment: Some(
            if cfg!(debug_assertions) {
                "development"
            } else {
                "production"
            }
            .into(),
        ),
        send_default_pii: false,
        max_breadcrumbs: 0,
        attach_stacktrace: true,
        before_send: Some(Arc::new(|mut event| {
            event.user = None;
            event.request = None;
            event.breadcrumbs.values.clear();
            Some(event)
        })),
        ..Default::default()
    })
}

/// Records that an unexpected backend error escaped an operation without
/// exporting its message, which may contain a library path or track metadata.
/// With `attach_stacktrace`, the call stack remains available for debugging.
pub fn capture_internal_error() {
    sentry::capture_message("internal backend error", sentry::Level::Error);
}

/// Records a player request that RBXport answered with generic `0x4003`.
///
/// The fields identify the protocol mismatch without exporting player names,
/// addresses, library paths, or request contents.
pub fn capture_unsupported_link_database_command(transaction: u32, command: u16, player: u8) {
    sentry::with_scope(
        |scope| {
            scope.set_tag("component", "link_database");
            scope.set_tag("command", format!("0x{command:04x}"));
            scope.set_tag("transaction", format!("0x{transaction:08x}"));
            scope.set_tag("player", player.to_string());
            scope.set_fingerprint(Some(&[
                "unsupported-link-database-command",
                &format!("{command:04x}"),
            ]));
        },
        || sentry::capture_message("unsupported Link database command", sentry::Level::Warning),
    );
}
