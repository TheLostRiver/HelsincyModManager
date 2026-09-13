use crate::dto::CommandErrorDto;
use crate::plugin_selection_dto::{
    PluginInventoryDto, PluginSelectionQueryDto, SetPluginSelectionDto,
};
use crate::replacement_commands::{parse_game_id, required_id};
use crate::state::AppState;
use hmm_core::{ModId, ModRevisionId, PackageFileId, PluginSelectionScope, ProfileId};
use tauri::State;

#[tauri::command]
pub async fn get_mod_plugin_selection(
    request: PluginSelectionQueryDto,
    state: State<'_, AppState>,
) -> Result<Option<PluginInventoryDto>, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let profile_id = ProfileId::new(required_id(
        request.profile_id,
        "plugin_profile_invalid",
        "profile is required",
    )?);
    let mod_id = ModId::new(required_id(
        request.mod_id,
        "plugin_mod_invalid",
        "Mod is required",
    )?);
    let revision_id = request
        .revision_id
        .map(|id| {
            required_id(id, "plugin_revision_invalid", "revision is required")
                .map(ModRevisionId::new)
        })
        .transpose()?;
    let service = state.plugin_selection.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let scope = service.resolve_scope(game_id, profile_id, mod_id, revision_id)?;
        service.inventory(&scope)
    })
    .await
    .map_err(|_| unavailable())?
    .map(|inventory| inventory.map(Into::into))
    .map_err(plugin_error)
}

#[tauri::command]
pub async fn set_mod_plugin_selection(
    request: SetPluginSelectionDto,
    state: State<'_, AppState>,
) -> Result<PluginInventoryDto, CommandErrorDto> {
    let scope = PluginSelectionScope {
        game_id: parse_game_id(request.game_id)?,
        profile_id: ProfileId::new(required_id(
            request.profile_id,
            "plugin_profile_invalid",
            "profile is required",
        )?),
        mod_id: ModId::new(required_id(
            request.mod_id,
            "plugin_mod_invalid",
            "Mod is required",
        )?),
        revision_id: ModRevisionId::new(required_id(
            request.revision_id,
            "plugin_revision_invalid",
            "revision is required",
        )?),
    };
    let inventory_id = required_id(
        request.inventory_id,
        "plugin_inventory_changed",
        "inventory is required",
    )?;
    let ids = request
        .selected_file_ids
        .into_iter()
        .map(PackageFileId::new)
        .collect::<Vec<_>>();
    let service = state.plugin_selection.clone();
    tauri::async_runtime::spawn_blocking(move || service.select(&scope, &inventory_id, &ids))
        .await
        .map_err(|_| unavailable())?
        .map(Into::into)
        .map_err(plugin_error)
}

pub(crate) fn plugin_error(error: hmm_app::PluginSelectionServiceError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

fn unavailable() -> CommandErrorDto {
    plugin_error(hmm_app::PluginSelectionServiceError::Unavailable)
}

pub(crate) fn expected_current_revision(
    value: Option<String>,
    mod_id: &ModId,
    workflow: &hmm_app::ReplacementWorkflowService,
) -> Result<Option<ModRevisionId>, CommandErrorDto> {
    let Some(value) = value else {
        return Ok(None);
    };
    let revision = ModRevisionId::new(required_id(
        value,
        "plugin_revision_invalid",
        "revision is required",
    )?);
    let current = workflow
        .current_install_revision(mod_id)
        .map_err(crate::replacement_commands::replacement_workflow_error_to_command_error)?;
    if current != revision {
        return Err(plugin_error(
            hmm_app::PluginSelectionServiceError::InventoryChanged,
        ));
    }
    Ok(Some(revision))
}
