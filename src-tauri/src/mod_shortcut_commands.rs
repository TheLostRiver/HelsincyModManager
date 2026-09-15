use crate::{dto::CommandErrorDto, state::AppState};
use hmm_app::ModShortcutError;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

fn shortcut_error(error: ModShortcutError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

#[tauri::command]
pub async fn open_mod_folder(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    let service = Arc::clone(&state.mod_shortcuts);
    tauri::async_runtime::spawn_blocking(move || service.open_mod_folder(&mod_id))
        .await
        .map_err(|_| shortcut_error(ModShortcutError::Unavailable))?
        .map_err(shortcut_error)
}

#[tauri::command]
pub async fn open_mod_nexus_page(
    mod_id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), CommandErrorDto> {
    let service = Arc::clone(&state.mod_shortcuts);
    let url = tauri::async_runtime::spawn_blocking(move || service.nexus_page_url(&mod_id))
        .await
        .map_err(|_| shortcut_error(ModShortcutError::Unavailable))?
        .map_err(shortcut_error)?;
    app_handle
        .opener()
        .open_url(url, None::<&str>)
        .map_err(|_| CommandErrorDto {
            code: "mod_nexus_open_failed".to_owned(),
            message: "The system browser could not be opened".to_owned(),
        })
}
