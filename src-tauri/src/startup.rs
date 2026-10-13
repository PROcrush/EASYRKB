//! Process-entry to frontend paint opportunities, including webview startup.
use std::sync::{atomic::{AtomicU8, Ordering}, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static REPORTED: AtomicU8 = AtomicU8::new(0);

const TEST_HEADLESS_ENV: &str = "RBXPORT_TEST_HEADLESS";

pub fn begin() { let _ = START.set(Instant::now()); }

/// Reveals a webview only after React has committed its window-specific UI.
/// All app windows start hidden, preventing the platform's empty webview
/// background from flashing before the first useful frame is available.
#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "Tauri injects the calling window by value")]
pub fn show_window(window: tauri::WebviewWindow) -> Result<(), String> {
    // The compiled-app integration suites still need the webview to render so
    // their test port can evaluate the page, but the CDJ rigs do not need to
    // put that window on the operator's desktop. Requiring the debug-only test
    // port as well as the explicit opt-in keeps an inherited environment
    // variable from hiding an ordinary app launch.
    if cfg!(debug_assertions)
        && std::env::var_os(crate::test_port::PORT_ENV).is_some()
        && std::env::var_os(TEST_HEADLESS_ENV).is_some()
    {
        return Ok(());
    }
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, reason = "a Tauri command's injected window and deserialized argument are owned")]
pub fn startup_milestone(window: tauri::WebviewWindow, phase: String) {
    if window.label() != "main" { return; }
    let flag = match phase.as_str() {
        "shell-painted" => 1,
        "first-rows-painted" => 2,
        _ => return,
    };
    if REPORTED.fetch_or(flag, Ordering::Relaxed) & flag != 0 { return; }
    if let Some(start) = START.get() {
        tracing::debug!(phase, elapsed_ms = start.elapsed().as_millis(), "startup milestone");
    }
}
