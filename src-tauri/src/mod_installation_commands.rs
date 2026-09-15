use crate::{dto::CommandErrorDto, state::AppState};
use hmm_core::{GameId, ModInstallationContext, ProfileId};
use hmm_ports::ModInstallationScopeError;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallationContextDto {
    game_id: String,
    installation_id: String,
    scope_id: String,
}

impl From<ModInstallationContext> for ModInstallationContextDto {
    fn from(value: ModInstallationContext) -> Self {
        Self {
            game_id: value.game_id.as_str().to_owned(),
            installation_id: value.installation_id,
            scope_id: value.scope_id.as_str().to_owned(),
        }
    }
}

#[tauri::command]
pub async fn get_mod_installation_context(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<ModInstallationContextDto, CommandErrorDto> {
    let game_id = crate::replacement_commands::parse_game_id(game_id)?;
    let service = Arc::clone(&state.mod_installation_scope);
    tauri::async_runtime::spawn_blocking(move || service.context(&game_id))
        .await
        .map_err(|_| scope_error(ModInstallationScopeError::Unavailable))?
        .map(Into::into)
        .map_err(scope_error)
}

pub(crate) fn scope_error(error: ModInstallationScopeError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

pub(crate) fn require_scope(
    state: &AppState,
    game_id: &GameId,
    scope_id: &ProfileId,
) -> Result<(), CommandErrorDto> {
    state
        .mod_installation_scope
        .require_current(game_id, scope_id)
        .map(|_| ())
        .map_err(scope_error)
}

pub(crate) fn require_optional_scope(
    state: &AppState,
    game_id: &GameId,
    scope_id: Option<&ProfileId>,
) -> Result<(), CommandErrorDto> {
    if let Some(scope_id) = scope_id {
        require_scope(state, game_id, scope_id)?;
    }
    Ok(())
}
