use std::sync::Arc;

use hmm_app::{
    AnalyzeImportedReplacementRequest, PlannedInitialRetargetInstall,
    PreviewInitialRetargetInstallRequest, ReplacementServiceError, ReplacementWorkflowError,
    RetargetInstallTaskService, StartRetargetInstallTaskRequest, TaskProgressEvent, TaskStarted,
};
use hmm_core::{
    FileLayer, GameId, ModId, ProfileId, ReplacementAnalysis, ReplacementTarget,
    ReplacementTargetId, ReplacementWarning,
};
use tauri::{AppHandle, State};

use crate::dto::{
    AnalyzeImportedModReplacementRequestDto, CommandErrorDto, InitialRetargetInstallPreviewDto,
    ListReplacementTargetsRequestDto, PreviewInitialRetargetInstallRequestDto,
    ReplacementAnalysisDto, ReplacementSourceDto, ReplacementTargetDto, ReplacementWarningDto,
    RetargetActionPreviewDto, StartRetargetInstallTaskRequestDto, TaskStartedDto,
};
use crate::state::AppState;
use crate::task_events::emit_task_progress;

const RETARGET_QUEUED_PHASE: &str = "install.retarget.queued";

#[tauri::command]
pub fn list_replacement_targets(
    request: ListReplacementTargetsRequestDto,
    state: State<'_, AppState>,
) -> Result<Vec<ReplacementTargetDto>, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    state
        .replacement_workflow
        .list_targets(&game_id, request.query.as_deref())
        .map(|targets| targets.into_iter().map(Into::into).collect())
        .map_err(replacement_workflow_error_to_command_error)
}

#[tauri::command]
pub fn analyze_imported_mod_replacement(
    request: AnalyzeImportedModReplacementRequestDto,
    state: State<'_, AppState>,
) -> Result<ReplacementAnalysisDto, CommandErrorDto> {
    let request = analyze_request_from_dto(request)?;
    state
        .replacement_workflow
        .analyze_imported_mod(request)
        .map(Into::into)
        .map_err(replacement_workflow_error_to_command_error)
}

#[tauri::command]
pub fn preview_initial_retarget_install(
    request: PreviewInitialRetargetInstallRequestDto,
    state: State<'_, AppState>,
) -> Result<InitialRetargetInstallPreviewDto, CommandErrorDto> {
    let request = preview_request_from_dto(request)?;
    state
        .replacement_workflow
        .preview_initial_install(request)
        .map(Into::into)
        .map_err(replacement_workflow_error_to_command_error)
}

#[tauri::command]
pub fn start_retarget_install_task(
    request: StartRetargetInstallTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = queue_retarget_install_task(&state.retarget_install_tasks, request)?;
    let _ = emit_task_progress(&app_handle, queued_event(&task).into());
    spawn_runner(
        Arc::clone(&state.retarget_install_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );
    Ok(task.into())
}

fn queue_retarget_install_task(
    task_service: &RetargetInstallTaskService,
    request: StartRetargetInstallTaskRequest,
) -> Result<TaskStarted, CommandErrorDto> {
    task_service
        .start_retarget_install_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)
}

fn spawn_runner(
    runner: Arc<hmm_app::RetargetInstallTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartRetargetInstallTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_retarget_install_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };
        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
}

fn queued_event(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        RETARGET_QUEUED_PHASE,
    )
}

fn analyze_request_from_dto(
    request: AnalyzeImportedModReplacementRequestDto,
) -> Result<AnalyzeImportedReplacementRequest, CommandErrorDto> {
    Ok(AnalyzeImportedReplacementRequest {
        game_id: parse_game_id(request.game_id)?,
        mod_id: ModId::new(required_id(
            request.mod_id,
            "replacement_mod_id_invalid",
            "Mod id is required",
        )?),
    })
}

fn preview_request_from_dto(
    request: PreviewInitialRetargetInstallRequestDto,
) -> Result<PreviewInitialRetargetInstallRequest, CommandErrorDto> {
    Ok(PreviewInitialRetargetInstallRequest {
        game_id: parse_game_id(request.game_id)?,
        profile_id: ProfileId::new(required_id(
            request.profile_id,
            "replacement_profile_id_invalid",
            "profile id is required",
        )?),
        mod_id: ModId::new(required_id(
            request.mod_id,
            "replacement_mod_id_invalid",
            "Mod id is required",
        )?),
        target_id: parse_target_id(request.target_id)?,
        layer: FileLayer::new(
            required_id(
                request.layer_name,
                "replacement_layer_invalid",
                "layer name is required",
            )?,
            request.layer_priority,
        ),
    })
}

fn start_request_from_dto(
    request: StartRetargetInstallTaskRequestDto,
) -> Result<StartRetargetInstallTaskRequest, CommandErrorDto> {
    let preview = preview_request_from_dto(PreviewInitialRetargetInstallRequestDto {
        game_id: request.game_id,
        profile_id: request.profile_id,
        mod_id: request.mod_id,
        target_id: request.target_id,
        layer_name: request.layer_name,
        layer_priority: request.layer_priority,
    })?;
    Ok(StartRetargetInstallTaskRequest {
        game_id: preview.game_id,
        profile_id: preview.profile_id,
        mod_id: preview.mod_id,
        target_id: preview.target_id,
        layer: preview.layer,
    })
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|_| CommandErrorDto {
        code: "replacement_unsupported_game".to_owned(),
        message: "replacement is unsupported for this game".to_owned(),
    })
}

fn parse_target_id(value: String) -> Result<ReplacementTargetId, CommandErrorDto> {
    ReplacementTargetId::parse(value).map_err(|_| CommandErrorDto {
        code: "replacement_target_id_invalid".to_owned(),
        message: "replacement target id is invalid".to_owned(),
    })
}

fn required_id(
    value: String,
    code: &'static str,
    message: &'static str,
) -> Result<String, CommandErrorDto> {
    let value = value.trim();
    if value.is_empty() {
        Err(CommandErrorDto {
            code: code.to_owned(),
            message: message.to_owned(),
        })
    } else {
        Ok(value.to_owned())
    }
}

fn replacement_workflow_error_to_command_error(error: ReplacementWorkflowError) -> CommandErrorDto {
    let (code, message) = match error {
        ReplacementWorkflowError::UnsupportedGame => (
            "replacement_unsupported_game",
            "replacement is unsupported for this game",
        ),
        ReplacementWorkflowError::CatalogUnavailable => (
            "replacement_target_catalog_unavailable",
            "replacement target catalog is unavailable",
        ),
        ReplacementWorkflowError::TargetNotFound => (
            "replacement_target_not_found",
            "replacement target was not found",
        ),
        ReplacementWorkflowError::ModRepositoryUnavailable
        | ReplacementWorkflowError::RevisionNotFound => (
            "replacement_package_unavailable",
            "imported Mod package is unavailable",
        ),
        ReplacementWorkflowError::ModNotFound => {
            ("replacement_mod_not_found", "imported Mod was not found")
        }
        ReplacementWorkflowError::SandboxUnavailable
        | ReplacementWorkflowError::PackageFilesUnavailable => (
            "replacement_package_unavailable",
            "imported Mod package is unavailable",
        ),
        ReplacementWorkflowError::SourceNotRetargetable => (
            "replacement_source_not_retargetable",
            "imported Mod does not contain one supported replacement source",
        ),
        ReplacementWorkflowError::InstallStatusUnavailable => (
            "replacement_install_state_unavailable",
            "replacement install state is unavailable",
        ),
        ReplacementWorkflowError::InitialInstallBlocked { status: _ } => (
            "replacement_initial_install_blocked",
            "initial replacement install is not allowed in the current state",
        ),
        ReplacementWorkflowError::BindingUnavailable
        | ReplacementWorkflowError::PlanUnavailable => (
            "replacement_preview_unavailable",
            "replacement preview is unavailable",
        ),
        ReplacementWorkflowError::Analysis(error) => return analysis_error_to_command_error(error),
    };
    CommandErrorDto {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn analysis_error_to_command_error(error: ReplacementServiceError) -> CommandErrorDto {
    let code = match error {
        ReplacementServiceError::UnsupportedGame => "replacement_unsupported_game",
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::UnrecognizedSourceSlot
            | hmm_ports::ReplacementAdapterError::AmbiguousSourceSlot
            | hmm_ports::ReplacementAdapterError::SourceBindingMismatch,
        ) => "replacement_source_not_retargetable",
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::TargetCatalogMissing { .. }
            | hmm_ports::ReplacementAdapterError::TargetCatalogUnavailable,
        ) => "replacement_target_catalog_unavailable",
        ReplacementServiceError::Adapter(_) => "replacement_analysis_unavailable",
    };
    CommandErrorDto {
        code: code.to_owned(),
        message: "replacement analysis is unavailable".to_owned(),
    }
}

impl From<ReplacementTarget> for ReplacementTargetDto {
    fn from(target: ReplacementTarget) -> Self {
        let display_name = target
            .display_name()
            .get("zh_cn")
            .or_else(|| target.display_name().get("en"))
            .or_else(|| target.display_name().values().next())
            .unwrap_or(target.internal_id())
            .to_owned();
        let secondary_name = target
            .display_name()
            .get("en")
            .filter(|name| *name != display_name.as_str())
            .map(str::to_owned);
        Self {
            id: target.id().as_str().to_owned(),
            game_id: target.game_id().as_str().to_owned(),
            target_type: target.target_type().as_str().to_owned(),
            display_name,
            secondary_name,
            aliases: target.aliases().to_vec(),
            internal_id: target.internal_id().to_owned(),
            metadata: target.metadata().clone(),
        }
    }
}

impl From<ReplacementAnalysis> for ReplacementAnalysisDto {
    fn from(analysis: ReplacementAnalysis) -> Self {
        Self {
            game_id: analysis.game_id().as_str().to_owned(),
            retargetable: analysis.is_retargetable(),
            matched_asset_count: analysis.matched_asset_count(),
            sources: analysis
                .sources()
                .iter()
                .map(|source| ReplacementSourceDto {
                    id: source.id().as_str().to_owned(),
                    source_type: source.source_type().as_str().to_owned(),
                    internal_id: source.internal_id().to_owned(),
                    path_family: source.path_family().to_owned(),
                    supported: source.is_supported(),
                })
                .collect(),
            warnings: analysis
                .warnings()
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<ReplacementWarning> for ReplacementWarningDto {
    fn from(warning: ReplacementWarning) -> Self {
        match warning {
            ReplacementWarning::NoSupportedAssets => Self::NoSupportedAssets,
            ReplacementWarning::MultipleSources => Self::MultipleSources,
            ReplacementWarning::UnsupportedSource => Self::UnsupportedSource,
            ReplacementWarning::SourceMatchesTarget => Self::SourceMatchesTarget,
        }
    }
}

impl From<PlannedInitialRetargetInstall> for InitialRetargetInstallPreviewDto {
    fn from(planned: PlannedInitialRetargetInstall) -> Self {
        Self {
            analysis: planned.analysis().clone().into(),
            target: planned.target().clone().into(),
            actions: planned
                .retarget_plan()
                .actions()
                .iter()
                .map(|action| RetargetActionPreviewDto {
                    source_relative_path: action.source_relative_path().as_str().to_owned(),
                    target_relative_path: action.target_relative_path().as_str().to_owned(),
                    source_internal_id: action.source_internal_id().to_owned(),
                    target_internal_id: action.target_internal_id().to_owned(),
                    source_path_family: action.source_path_family().to_owned(),
                    target_path_family: action.target_path_family().to_owned(),
                })
                .collect(),
            warnings: planned
                .retarget_plan()
                .warnings()
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            install_plan: planned.install_plan().clone().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_mapping_rejects_paths_and_accepts_only_stable_ids() {
        let request: PreviewInitialRetargetInstallRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0
        }))
        .expect("deserialize stable ids");
        let mapped = preview_request_from_dto(request).expect("map stable ids");
        assert_eq!(mapped.mod_id.as_str(), "mod-a");
        assert_eq!(mapped.target_id.as_str(), "mhw:armor:fatalis-alpha");

        let forbidden = serde_json::from_value::<PreviewInitialRetargetInstallRequestDto>(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0,
            "sandboxPath": "C:/forbidden"
        }));
        assert!(forbidden.is_err());
    }

    #[test]
    fn workflow_errors_map_to_stable_codes_without_status_details() {
        let blocked = replacement_workflow_error_to_command_error(
            ReplacementWorkflowError::InitialInstallBlocked {
                status: hmm_app::InstallRecoveryStatus::RollbackRequired,
            },
        );
        assert_eq!(blocked.code, "replacement_initial_install_blocked");
        assert!(!blocked.message.contains("RollbackRequired"));
    }

    #[test]
    fn start_task_queueing_does_not_require_a_workflow_preview() {
        let task_manager = Arc::new(hmm_app::TaskManager::new());
        let task_service = hmm_app::RetargetInstallTaskService::new(task_manager);
        let request = start_request_from_dto(StartRetargetInstallTaskRequestDto {
            game_id: "mhw".to_owned(),
            profile_id: "profile-a".to_owned(),
            mod_id: "mod-a".to_owned(),
            target_id: "mhw:armor:fatalis-alpha".to_owned(),
            layer_name: "base".to_owned(),
            layer_priority: 0,
        })
        .expect("valid controlled request");

        let task = queue_retarget_install_task(&task_service, request).expect("queue task");

        assert_eq!(task.kind, hmm_app::TaskKind::Install);
        assert_eq!(task.status, hmm_app::TaskStatus::Queued);
    }
}
