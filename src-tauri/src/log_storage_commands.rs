use crate::dto::{CommandErrorDto, LogStorageSettingsDto};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_log_storage_settings(
    state: State<'_, AppState>,
) -> Result<LogStorageSettingsDto, CommandErrorDto> {
    state
        .app_settings
        .get_settings()
        .map(Into::into)
        .map_err(CommandErrorDto::from_app_settings_service_error)
}

#[tauri::command]
pub fn set_log_storage_settings(
    max_bytes: Option<u64>,
    state: State<'_, AppState>,
) -> Result<LogStorageSettingsDto, CommandErrorDto> {
    state
        .app_settings
        .update_log_storage_settings(max_bytes)
        .map(Into::into)
        .map_err(CommandErrorDto::from_app_settings_service_error)
}
