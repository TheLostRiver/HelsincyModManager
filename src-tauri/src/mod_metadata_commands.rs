use crate::dto::CommandErrorDto;
use crate::state::AppState;
use hmm_app::UpdateModMetadataRequest;
use tauri::State;

#[tauri::command]
pub fn update_mod_metadata(
    mod_id: String,
    display_name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    description: Option<String>,
    nexus_mod_id: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;

    state
        .mod_metadata
        .update_metadata(UpdateModMetadataRequest {
            mod_id,
            display_name,
            author,
            version,
            description,
            nexus_mod_id,
        })
        .map_err(|_| metadata_unavailable_error())
}

#[tauri::command]
pub fn delete_mod_metadata(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;

    state
        .mod_metadata
        .delete_metadata(&mod_id)
        .map_err(|_| metadata_unavailable_error())
}

fn parse_mod_id(value: String) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "mod_id_empty".to_owned(),
            message: "mod id cannot be empty".to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

fn metadata_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_metadata_unavailable".to_owned(),
        message: "mod metadata storage is unavailable".to_owned(),
    }
}
