use crate::dto::CommandErrorDto;
use hmm_app::{ModDeletionError, ModDeletionPreview, ModDeletionResult};
use hmm_core::ModId;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDeletionPreviewDto {
    pub mod_id: String,
    pub display_name: String,
    pub revision_count: usize,
    pub category_labels: Vec<String>,
    pub affected_profiles: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDeletionResultDto {
    pub mod_id: String,
    pub removed_revision_count: usize,
    pub removed_package_ids: Vec<String>,
}

fn deletion_error_to_command_error(error: ModDeletionError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

#[tauri::command]
pub fn preview_mod_deletion(
    mod_id: String,
    state: State<'_, crate::AppState>,
) -> Result<ModDeletionPreviewDto, CommandErrorDto> {
    let preview = state
        .mod_deletion
        .preview_mod_deletion(&ModId::new(&mod_id))
        .map_err(deletion_error_to_command_error)?;
    let ModDeletionPreview {
        mod_id,
        display_name,
        revision_count,
        category_labels,
        affected_profiles,
    } = preview;
    Ok(ModDeletionPreviewDto {
        mod_id: mod_id.as_str().to_owned(),
        display_name,
        revision_count,
        category_labels,
        affected_profiles,
    })
}

#[tauri::command]
pub fn delete_mod_from_library(
    mod_id: String,
    state: State<'_, crate::AppState>,
) -> Result<ModDeletionResultDto, CommandErrorDto> {
    let result = state
        .mod_deletion
        .delete_mod(&ModId::new(&mod_id))
        .map_err(deletion_error_to_command_error)?;
    let ModDeletionResult {
        mod_id,
        removed_revision_count,
        removed_package_ids,
    } = result;
    Ok(ModDeletionResultDto {
        mod_id: mod_id.as_str().to_owned(),
        removed_revision_count,
        removed_package_ids,
    })
}
