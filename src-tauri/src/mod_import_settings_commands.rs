//! #275 slice 4: the "delete the source archive after import" preference. Reading and writing
//! the flag is all these commands do; the deletion itself happens inside the import runner,
//! after the catalog write, through the fingerprint-checked consumer.

use crate::dto::CommandErrorDto;
use crate::state::AppState;
use hmm_ports::AppSettings;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModImportSettingsDto {
    pub delete_archive_after_import: bool,
}

impl From<AppSettings> for ModImportSettingsDto {
    fn from(settings: AppSettings) -> Self {
        Self {
            delete_archive_after_import: settings.delete_archive_after_import,
        }
    }
}

#[tauri::command]
pub fn get_mod_import_settings(
    state: State<'_, AppState>,
) -> Result<ModImportSettingsDto, CommandErrorDto> {
    state
        .app_settings
        .get_settings()
        .map(Into::into)
        .map_err(CommandErrorDto::from_app_settings_service_error)
}

#[tauri::command]
pub fn set_mod_import_settings(
    delete_archive_after_import: bool,
    state: State<'_, AppState>,
) -> Result<ModImportSettingsDto, CommandErrorDto> {
    state
        .app_settings
        .update_delete_archive_after_import(delete_archive_after_import)
        .map(Into::into)
        .map_err(CommandErrorDto::from_app_settings_service_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_exposes_only_the_flag_in_camel_case() {
        let dto = ModImportSettingsDto::from(AppSettings {
            delete_archive_after_import: true,
            ..AppSettings::default()
        });
        let value = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(
            value,
            serde_json::json!({ "deleteArchiveAfterImport": true })
        );
        assert!(!ModImportSettingsDto::from(AppSettings::default()).delete_archive_after_import);
    }
}
