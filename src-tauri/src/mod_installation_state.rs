//! Narrow, versioned display observations. Integrity scans and write authorization
//! retain their existing commands and application services.
use crate::{
    dto::{CommandErrorDto, InstallManifestStatusSummaryDto},
    state::AppState,
};
use hmm_app::{
    ModInstallationStateObserver, ModInstallationStateUpdate, MAX_MOD_INSTALLATION_STATE_IDS,
};
use hmm_core::{GameId, ModId, ProfileId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

pub const MOD_INSTALLATION_STATE_EVENT: &str = "hmm://mod-installation-state";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModInstallationStateRequestDto {
    game_id: String,
    profile_id: String,
    mod_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallationStateUpdateDto {
    game_id: String,
    profile_id: String,
    epoch: String,
    revision: u32,
    reset: bool,
    available: bool,
    mod_ids: Vec<String>,
    summaries: Vec<InstallManifestStatusSummaryDto>,
}

impl From<ModInstallationStateUpdate> for ModInstallationStateUpdateDto {
    fn from(update: ModInstallationStateUpdate) -> Self {
        Self {
            game_id: update.game_id.as_str().to_owned(),
            profile_id: update.profile_id.as_str().to_owned(),
            epoch: update.epoch,
            revision: update.revision,
            reset: update.reset,
            available: update.available,
            mod_ids: update
                .mod_ids
                .into_iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            summaries: update.summaries.into_iter().map(Into::into).collect(),
        }
    }
}

fn unavailable() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_installation_state_unavailable".to_owned(),
        message: "Mod installation state is unavailable".to_owned(),
    }
}

fn parse_request(
    request: ModInstallationStateRequestDto,
) -> Result<(GameId, ProfileId, Vec<ModId>), CommandErrorDto> {
    let valid_id = |id: &str| {
        !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control) && id.trim() == id
    };
    if !valid_id(&request.profile_id)
        || request.mod_ids.len() > MAX_MOD_INSTALLATION_STATE_IDS
        || request.mod_ids.iter().any(|id| !valid_id(id))
    {
        return Err(unavailable());
    }
    Ok((
        GameId::parse(request.game_id).map_err(|_| unavailable())?,
        ProfileId::new(request.profile_id),
        request.mod_ids.into_iter().map(ModId::new).collect(),
    ))
}

#[tauri::command]
pub async fn get_mod_installation_states(
    request: ModInstallationStateRequestDto,
    state: State<'_, AppState>,
) -> Result<ModInstallationStateUpdateDto, CommandErrorDto> {
    let (game_id, profile_id, mod_ids) = parse_request(request)?;
    let scope = Arc::clone(&state.mod_installation_scope);
    let session = Arc::clone(&state.mod_installation_state_session);
    tauri::async_runtime::spawn_blocking(move || {
        scope
            .require_current(&game_id, &profile_id)
            .map_err(crate::mod_installation_commands::scope_error)?;
        session
            .read(&game_id, &profile_id, &mod_ids)
            .map(Into::into)
            .map_err(|_| unavailable())
    })
    .await
    .map_err(|_| unavailable())?
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModInstallationStateEventDto {
    task_id: String,
    #[serde(flatten)]
    update: ModInstallationStateUpdateDto,
}

pub(crate) struct TauriModInstallationStateObserver {
    app_handle: AppHandle,
}

impl TauriModInstallationStateObserver {
    pub(crate) fn new(app_handle: &AppHandle) -> Self {
        Self {
            app_handle: app_handle.clone(),
        }
    }
}

impl ModInstallationStateObserver for TauriModInstallationStateObserver {
    fn state_changed(
        &self,
        task_id: &str,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) {
        let state = self.app_handle.state::<AppState>();
        let published = state
            .mod_installation_state_session
            .read(game_id, profile_id, std::slice::from_ref(mod_id))
            .ok()
            .and_then(|update| {
                self.app_handle
                    .emit(
                        MOD_INSTALLATION_STATE_EVENT,
                        ModInstallationStateEventDto {
                            task_id: task_id.to_owned(),
                            update: update.into(),
                        },
                    )
                    .ok()
            });
        if published.is_none() {
            hmm_infra::emit_safe_app_log(
                hmm_infra::AppLogEvent::warning("mod_installation_state.publish_failed")
                    .with_task_id(task_id.to_owned())
                    .with_error_code("mod_installation_state_unavailable"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observation_request_is_bounded_and_does_not_accept_filesystem_arguments() {
        let request = |ids| ModInstallationStateRequestDto {
            game_id: "mhw".to_owned(),
            profile_id: "default".to_owned(),
            mod_ids: ids,
        };
        assert!(parse_request(request(vec!["mod-a".to_owned()])).is_ok());
        assert!(parse_request(request(vec!["".to_owned()])).is_err());
        assert!(parse_request(request(vec![
            "a".to_owned();
            MAX_MOD_INSTALLATION_STATE_IDS + 1
        ]))
        .is_err());
        assert!(
            serde_json::from_value::<ModInstallationStateRequestDto>(serde_json::json!({
                "gameId": "mhw", "profileId": "default", "modIds": [], "gameRoot": "arbitrary"
            }))
            .is_err()
        );
    }
}
