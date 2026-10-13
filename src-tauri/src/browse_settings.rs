//! Read-only access to rekordbox's browser layout for startup import.

#[tauri::command]
pub fn rekordbox_browse_settings() -> Option<String> {
    let file = rbl_core::paths::rekordbox_settings_dir()?.join("browseSetting.xml");
    // A normal file is small. Cap a corrupt or unexpected file before passing
    // it to the webview's XML parser.
    if std::fs::metadata(&file).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    std::fs::read_to_string(file).ok()
}
