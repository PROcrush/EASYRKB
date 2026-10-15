//! Standalone local conversion; does not open or modify the music library.
use crate::{AppError, AppResult, ErrorKind};
use serde::Serialize;
use std::path::Path;
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Converted {
    output_path: String,
    format: String,
    bytes: u64,
}

#[tauri::command]
pub async fn ncm_convert_file(source: String, directory: String) -> AppResult<Converted> {
    crate::commands::blocking("ncm_convert_file", move || {
        let result = rbl_ncm::convert_file(Path::new(&source), Path::new(&directory))
            .map_err(|error| AppError::new(ErrorKind::Malformed, error.to_string()))?;
        Ok(Converted {
            output_path: result.path.to_string_lossy().into_owned(),
            format: result.format.into(),
            bytes: result.bytes,
        })
    })
    .await
}

#[tauri::command]
pub fn ncm_open_output<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    directory: String,
) -> AppResult<()> {
    if !Path::new(&directory).is_dir() {
        return Err(AppError::new(
            ErrorKind::NotFound,
            "Choose an existing output folder.",
        ));
    }
    app.opener()
        .open_path(directory, None::<&str>)
        .map_err(|error| AppError::new(ErrorKind::Internal, error.to_string()))
}
