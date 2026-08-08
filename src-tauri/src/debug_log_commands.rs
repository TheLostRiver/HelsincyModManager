use crate::dto::{CommandErrorDto, DebugLogSettingsDto};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_debug_log_settings(
    state: State<'_, AppState>,
) -> Result<DebugLogSettingsDto, CommandErrorDto> {
    state
        .app_settings
        .get_settings()
        .map(Into::into)
        .map_err(CommandErrorDto::from_app_settings_service_error)
}

#[tauri::command]
pub fn set_debug_log_settings(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<DebugLogSettingsDto, CommandErrorDto> {
    state
        .app_settings
        .update_debug_log_enabled(enabled)
        .map(Into::into)
        .map_err(CommandErrorDto::from_app_settings_service_error)
}
