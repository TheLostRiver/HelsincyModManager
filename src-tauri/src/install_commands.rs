use crate::dto::{
    CommandErrorDto, InstallManifestStatusRequestDto, InstallManifestStatusSummaryDto,
    InstallPlanPreviewDto, InstallRecoveryActionKindDto, InstallRecoveryActionPreviewDto,
    InstallRecoveryActionPreviewRequestDto, InstallRecoveryScanRequestDto,
    InstallRecoverySummaryDto, PreviewImportedModInstallPlanRequestDto,
    PreviewInstallPlanFileInputDto, PreviewInstallPlanRequestDto, StartInstallTaskRequestDto,
    StartRecoveryActionTaskRequestDto, StartUninstallTaskRequestDto, TaskStartedDto,
};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
#[cfg(test)]
use hmm_app::InstallRecoveryStatus;
use hmm_app::{
    BuildImportedModInstallPlanRequest, BuildInstallPlanRequest, InstallManifestQueryError,
    InstallManifestQueryRequest, InstallManifestStatusSummary, InstallPlanFile,
    InstallPlanningError, InstallRecoveryActionKind, InstallRecoveryActionPreviewError,
    InstallRecoveryActionPreviewRequest, InstallRecoveryScanError, InstallRecoveryScanRequest,
    InstallRecoverySummary, StartInstallTaskRequest, StartRecoveryActionTaskRequest,
    StartUninstallTaskRequest, TaskProgressEvent, TaskStarted,
};
use hmm_core::{FileLayer, GameId, InstallTargetPathError, ModId, PackageFileId, ProfileId};
use std::sync::Arc;
use tauri::{AppHandle, State};

const INSTALL_QUEUED_PHASE: &str = "install.queued";
const INSTALL_UNINSTALL_QUEUED_PHASE: &str = "install.uninstall.queued";
const INSTALL_RECOVERY_QUEUED_PHASE: &str = "install.recovery.queued";

#[tauri::command]
pub fn preview_install_plan(
    request: PreviewInstallPlanRequestDto,
    state: State<'_, AppState>,
) -> Result<InstallPlanPreviewDto, CommandErrorDto> {
    let request = build_install_plan_request_from_dto(request);
    let plan = state
        .install_planning
        .build_plan(request)
        .map_err(install_planning_error_to_command_error)?;

    Ok(plan.into())
}

#[tauri::command]
pub fn preview_imported_mod_install_plan(
    request: PreviewImportedModInstallPlanRequestDto,
    state: State<'_, AppState>,
) -> Result<InstallPlanPreviewDto, CommandErrorDto> {
    let request = imported_mod_install_plan_request_from_dto(request)?;
    let plan = state
        .install_planning
        .build_plan_from_imported_mod(request)
        .map_err(install_planning_error_to_command_error)?;

    Ok(plan.into())
}

#[tauri::command]
pub fn start_install_task(
    request: StartInstallTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_install_task_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = state
        .install_tasks
        .start_install_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)?;

    let _ = emit_task_progress(
        &app_handle,
        queued_event_for_started_install_task(&task).into(),
    );
    spawn_install_runner(
        Arc::clone(&state.install_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );

    Ok(task.into())
}

#[tauri::command]
pub fn start_uninstall_task(
    request: StartUninstallTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_uninstall_task_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = state
        .uninstall_tasks
        .start_uninstall_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)?;

    let _ = emit_task_progress(
        &app_handle,
        queued_event_for_started_uninstall_task(&task).into(),
    );
    spawn_uninstall_runner(
        Arc::clone(&state.uninstall_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );

    Ok(task.into())
}

#[tauri::command]
pub fn get_install_manifest_status(
    request: InstallManifestStatusRequestDto,
    state: State<'_, AppState>,
) -> Result<Vec<InstallManifestStatusSummaryDto>, CommandErrorDto> {
    let (game_id, request) = install_manifest_status_request_from_dto(request)?;
    let summaries = if let Some(game_id) = game_id {
        state
            .install_recovery_scanner
            .scan(
                game_id,
                InstallRecoveryScanRequest {
                    profile_id: request.profile_id,
                    mod_ids: request.mod_ids,
                },
            )
            .map_err(install_recovery_scan_error_to_command_error)?
            .into_iter()
            .map(recovery_summary_to_manifest_status_summary)
            .collect()
    } else {
        state
            .install_manifest_query
            .query_statuses(request)
            .map_err(install_manifest_query_error_to_command_error)?
    };

    Ok(summaries.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub fn scan_install_recovery(
    request: InstallRecoveryScanRequestDto,
    state: State<'_, AppState>,
) -> Result<Vec<InstallRecoverySummaryDto>, CommandErrorDto> {
    let (game_id, request) = install_recovery_scan_request_from_dto(request)?;
    let summaries = state
        .install_recovery_scanner
        .scan(game_id, request)
        .map_err(install_recovery_scan_error_to_command_error)?;

    Ok(summaries.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub fn preview_recovery_action(
    request: InstallRecoveryActionPreviewRequestDto,
    state: State<'_, AppState>,
) -> Result<InstallRecoveryActionPreviewDto, CommandErrorDto> {
    let (game_id, request) = install_recovery_action_preview_request_from_dto(request)?;
    let preview = state
        .install_recovery_action_previewer
        .preview(game_id, request)
        .map_err(install_recovery_action_preview_error_to_command_error)?;

    Ok(preview.into())
}

#[tauri::command]
pub fn start_recovery_action_task(
    request: StartRecoveryActionTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_recovery_action_task_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = state
        .recovery_action_tasks
        .start_recovery_action_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)?;

    let _ = emit_task_progress(
        &app_handle,
        queued_event_for_started_recovery_action_task(&task).into(),
    );
    spawn_recovery_action_runner(
        Arc::clone(&state.recovery_action_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );

    Ok(task.into())
}

fn spawn_recovery_action_runner(
    runner: Arc<hmm_app::RecoveryActionTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartRecoveryActionTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_recovery_action_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
}

fn spawn_uninstall_runner(
    runner: Arc<hmm_app::UninstallTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartUninstallTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_uninstall_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
}

fn spawn_install_runner(
    runner: Arc<hmm_app::InstallTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartInstallTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_install_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
}

fn queued_event_for_started_uninstall_task(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        INSTALL_UNINSTALL_QUEUED_PHASE,
    )
}

fn queued_event_for_started_install_task(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        INSTALL_QUEUED_PHASE,
    )
}

fn queued_event_for_started_recovery_action_task(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        INSTALL_RECOVERY_QUEUED_PHASE,
    )
}

fn start_uninstall_task_request_from_dto(
    request: StartUninstallTaskRequestDto,
) -> Result<StartUninstallTaskRequest, CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;
    let mod_id = parse_non_empty_id(request.mod_id, "mod_id_empty", "mod id cannot be empty")?;
    let profile_id = parse_non_empty_id(
        request.profile_id,
        "profile_id_empty",
        "profile id cannot be empty",
    )?;

    Ok(StartUninstallTaskRequest {
        game_id,
        mod_id: ModId::new(mod_id),
        profile_id: ProfileId::new(profile_id),
    })
}

fn build_install_plan_request_from_dto(
    request: PreviewInstallPlanRequestDto,
) -> BuildInstallPlanRequest {
    BuildInstallPlanRequest {
        allowed_target_roots: request.allowed_target_roots,
        files: request
            .files
            .into_iter()
            .map(install_plan_file_from_dto)
            .collect(),
    }
}

fn imported_mod_install_plan_request_from_dto(
    request: PreviewImportedModInstallPlanRequestDto,
) -> Result<BuildImportedModInstallPlanRequest, CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;

    Ok(BuildImportedModInstallPlanRequest {
        game_id,
        mod_id: ModId::new(request.mod_id),
        layer: FileLayer::new(request.layer_name, request.layer_priority),
    })
}

fn start_install_task_request_from_dto(
    request: StartInstallTaskRequestDto,
) -> Result<StartInstallTaskRequest, CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;
    let mod_id = parse_non_empty_id(request.mod_id, "mod_id_empty", "mod id cannot be empty")?;
    let profile_id = parse_non_empty_id(
        request.profile_id,
        "profile_id_empty",
        "profile id cannot be empty",
    )?;
    let layer_name = parse_non_empty_id(
        request.layer_name,
        "layer_name_empty",
        "layer name cannot be empty",
    )?;

    Ok(StartInstallTaskRequest {
        game_id,
        mod_id: ModId::new(mod_id),
        profile_id: ProfileId::new(profile_id),
        layer: FileLayer::new(layer_name, request.layer_priority),
    })
}

fn install_manifest_status_request_from_dto(
    request: InstallManifestStatusRequestDto,
) -> Result<(Option<GameId>, InstallManifestQueryRequest), CommandErrorDto> {
    let profile_id = parse_non_empty_id(
        request.profile_id,
        "profile_id_empty",
        "profile id cannot be empty",
    )?;
    if request.mod_ids.is_empty() {
        return Err(CommandErrorDto {
            code: "mod_ids_empty".to_owned(),
            message: "mod ids cannot be empty".to_owned(),
        });
    }

    let mod_ids = request
        .mod_ids
        .into_iter()
        .map(|mod_id| parse_non_empty_id(mod_id, "mod_id_empty", "mod id cannot be empty"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(ModId::new)
        .collect();

    let game_id = request
        .game_id
        .map(|game_id| {
            GameId::parse(game_id).map_err(|_| CommandErrorDto {
                code: "game_id_invalid".to_owned(),
                message: "game id is invalid".to_owned(),
            })
        })
        .transpose()?;

    Ok((
        game_id,
        InstallManifestQueryRequest {
            profile_id: ProfileId::new(profile_id),
            mod_ids,
        },
    ))
}

fn recovery_summary_to_manifest_status_summary(
    summary: InstallRecoverySummary,
) -> InstallManifestStatusSummary {
    InstallManifestStatusSummary {
        profile_id: summary.profile_id,
        mod_id: summary.mod_id,
        status: hmm_app::InstallManifestStatus::from_recovery_status(summary.status),
        managed_file_count: summary.managed_file_count,
        backup_count: summary.backup_count,
    }
}

fn install_recovery_scan_request_from_dto(
    request: InstallRecoveryScanRequestDto,
) -> Result<(GameId, InstallRecoveryScanRequest), CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;
    let profile_id = parse_non_empty_id(
        request.profile_id,
        "profile_id_empty",
        "profile id cannot be empty",
    )?;
    let mod_ids = request
        .mod_ids
        .into_iter()
        .map(|mod_id| parse_non_empty_id(mod_id, "mod_id_empty", "mod id cannot be empty"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(ModId::new)
        .collect();

    Ok((
        game_id,
        InstallRecoveryScanRequest {
            profile_id: ProfileId::new(profile_id),
            mod_ids,
        },
    ))
}

fn install_recovery_action_preview_request_from_dto(
    request: InstallRecoveryActionPreviewRequestDto,
) -> Result<(GameId, InstallRecoveryActionPreviewRequest), CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;
    let profile_id = parse_non_empty_id(
        request.profile_id,
        "profile_id_empty",
        "profile id cannot be empty",
    )?;
    let mod_id = parse_non_empty_id(request.mod_id, "mod_id_empty", "mod id cannot be empty")?;

    Ok((
        game_id,
        InstallRecoveryActionPreviewRequest {
            profile_id: ProfileId::new(profile_id),
            mod_id: ModId::new(mod_id),
            action_kind: install_recovery_action_kind_from_dto(request.action_kind),
        },
    ))
}

fn start_recovery_action_task_request_from_dto(
    request: StartRecoveryActionTaskRequestDto,
) -> Result<StartRecoveryActionTaskRequest, CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;
    let profile_id = parse_non_empty_id(
        request.profile_id,
        "profile_id_empty",
        "profile id cannot be empty",
    )?;
    let mod_id = parse_non_empty_id(request.mod_id, "mod_id_empty", "mod id cannot be empty")?;

    Ok(StartRecoveryActionTaskRequest {
        game_id,
        profile_id: ProfileId::new(profile_id),
        mod_id: ModId::new(mod_id),
        action_kind: install_recovery_action_kind_from_dto(request.action_kind),
    })
}

fn install_recovery_action_kind_from_dto(
    action_kind: InstallRecoveryActionKindDto,
) -> InstallRecoveryActionKind {
    match action_kind {
        InstallRecoveryActionKindDto::RollbackInstall => InstallRecoveryActionKind::RollbackInstall,
        InstallRecoveryActionKindDto::ReconcileReinstall => {
            InstallRecoveryActionKind::ReconcileReinstall
        }
    }
}

fn parse_non_empty_id(
    value: String,
    code: &'static str,
    message: &'static str,
) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: code.to_owned(),
            message: message.to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

fn install_plan_file_from_dto(file: PreviewInstallPlanFileInputDto) -> InstallPlanFile {
    InstallPlanFile {
        mod_id: ModId::new(file.mod_id),
        package_file_id: PackageFileId::new(file.package_file_id),
        target_path: file.target_path,
        layer: FileLayer::new(file.layer_name, file.layer_priority),
    }
}

fn install_planning_error_to_command_error(error: InstallPlanningError) -> CommandErrorDto {
    match error {
        InstallPlanningError::InvalidTargetPath {
            package_file_id: _,
            source,
        } => CommandErrorDto {
            code: install_target_path_error_code(source).to_owned(),
            message: "install target path is invalid".to_owned(),
        },
        InstallPlanningError::ImportedModSourcesUnavailable => CommandErrorDto {
            code: "install_planning_sources_unavailable".to_owned(),
            message: "install planning sources are unavailable".to_owned(),
        },
        InstallPlanningError::GameAdapterNotFound { game_id: _ } => CommandErrorDto {
            code: "install_planning_game_adapter_not_found".to_owned(),
            message: "game adapter is unavailable for install planning".to_owned(),
        },
        InstallPlanningError::ImportedModNotFound { mod_id: _ } => CommandErrorDto {
            code: "install_planning_imported_mod_not_found".to_owned(),
            message: "imported mod was not found".to_owned(),
        },
        InstallPlanningError::ImportedModAnalysisUnavailable => CommandErrorDto {
            code: "install_planning_imported_mod_analysis_unavailable".to_owned(),
            message: "imported mod analysis is unavailable".to_owned(),
        },
        InstallPlanningError::ImportedModSandboxUnavailable => CommandErrorDto {
            code: "install_planning_imported_mod_sandbox_unavailable".to_owned(),
            message: "imported mod sandbox is unavailable".to_owned(),
        },
        InstallPlanningError::ImportedModFileScanUnavailable => CommandErrorDto {
            code: "install_planning_imported_mod_file_scan_unavailable".to_owned(),
            message: "imported mod files are unavailable".to_owned(),
        },
    }
}

fn install_manifest_query_error_to_command_error(
    error: InstallManifestQueryError,
) -> CommandErrorDto {
    match error {
        InstallManifestQueryError::ManifestUnavailable => CommandErrorDto {
            code: "install_manifest_unavailable".to_owned(),
            message: "install manifest status is unavailable".to_owned(),
        },
    }
}

fn install_recovery_scan_error_to_command_error(
    error: InstallRecoveryScanError,
) -> CommandErrorDto {
    match error {
        InstallRecoveryScanError::GameInstanceUnavailable => CommandErrorDto {
            code: "game_instance_unavailable".to_owned(),
            message: "game instance is unavailable".to_owned(),
        },
        InstallRecoveryScanError::ManifestUnavailable => CommandErrorDto {
            code: "install_recovery_unavailable".to_owned(),
            message: "install recovery scan is unavailable".to_owned(),
        },
    }
}

fn install_recovery_action_preview_error_to_command_error(
    error: InstallRecoveryActionPreviewError,
) -> CommandErrorDto {
    match error {
        InstallRecoveryActionPreviewError::GameInstanceUnavailable => CommandErrorDto {
            code: "game_instance_unavailable".to_owned(),
            message: "game instance is unavailable".to_owned(),
        },
        InstallRecoveryActionPreviewError::PreviewUnavailable => CommandErrorDto {
            code: "install_recovery_action_preview_unavailable".to_owned(),
            message: "install recovery action preview is unavailable".to_owned(),
        },
    }
}

fn install_target_path_error_code(error: InstallTargetPathError) -> &'static str {
    match error {
        InstallTargetPathError::Empty => "install_target_path_empty",
        InstallTargetPathError::Absolute => "install_target_path_absolute",
        InstallTargetPathError::ParentTraversal => "install_target_path_parent_traversal",
        InstallTargetPathError::WindowsDrivePrefix => "install_target_path_windows_drive_prefix",
        InstallTargetPathError::InvalidSegment => "install_target_path_invalid_segment",
        InstallTargetPathError::TargetRootNotAllowed { .. } => "install_target_root_not_allowed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{
        InstallPlanPreviewDto, PreviewImportedModInstallPlanRequestDto,
        PreviewInstallPlanFileInputDto, PreviewInstallPlanRequestDto,
        StartRecoveryActionTaskRequestDto, StartUninstallTaskRequestDto, TaskProgressEventDto,
    };
    use hmm_core::{
        FileLayer, InstallAction, InstallConflict, InstallFileProvider, InstallPlan,
        InstallTargetPath, InstallTargetPathError, ModId, PackageFileId,
    };
    use serde_json::{json, Value};

    #[test]
    fn preview_install_plan_request_deserializes_camel_case_fields() {
        let value = json!({
            "allowedTargetRoots": ["content"],
            "files": [{
                "modId": "mod-a",
                "packageFileId": "file-a",
                "targetPath": "content/models/player.mod3",
                "layerName": "base",
                "layerPriority": 10
            }]
        });

        let request: PreviewInstallPlanRequestDto =
            serde_json::from_value(value).expect("request should deserialize");

        assert_eq!(request.allowed_target_roots, vec!["content"]);
        assert_eq!(request.files[0].mod_id, "mod-a");
        assert_eq!(request.files[0].package_file_id, "file-a");
        assert_eq!(request.files[0].target_path, "content/models/player.mod3");
        assert_eq!(request.files[0].layer_name, "base");
        assert_eq!(request.files[0].layer_priority, 10);
    }

    #[test]
    fn preview_imported_mod_install_plan_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "modId": "mod-a",
            "layerName": "base",
            "layerPriority": 10
        });

        let request: PreviewImportedModInstallPlanRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let app_request = imported_mod_install_plan_request_from_dto(request)
            .expect("valid ids should map to app request");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.mod_id.as_str(), "mod-a");
        assert_eq!(app_request.layer.name, "base");
        assert_eq!(app_request.layer.priority, 10);
    }

    #[test]
    fn start_install_task_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "modId": "mod-a",
            "profileId": "default",
            "layerName": "base",
            "layerPriority": 10
        });

        let request: StartInstallTaskRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let app_request =
            start_install_task_request_from_dto(request).expect("valid ids should map");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.mod_id.as_str(), "mod-a");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(app_request.layer.name, "base");
        assert_eq!(app_request.layer.priority, 10);
    }

    #[test]
    fn start_uninstall_task_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "modId": "mod-a",
            "profileId": "default"
        });

        let request: StartUninstallTaskRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let app_request =
            start_uninstall_task_request_from_dto(request).expect("valid ids should map");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.mod_id.as_str(), "mod-a");
        assert_eq!(app_request.profile_id.as_str(), "default");
    }

    #[test]
    fn install_manifest_status_request_deserializes_without_paths() {
        let value = json!({
            "profileId": "default",
            "modIds": ["mod-a", "mod-b"]
        });

        let request: crate::dto::InstallManifestStatusRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let (game_id, app_request) =
            install_manifest_status_request_from_dto(request).expect("valid ids should map");

        assert!(game_id.is_none());
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(
            app_request
                .mod_ids
                .iter()
                .map(|mod_id| mod_id.as_str())
                .collect::<Vec<_>>(),
            vec!["mod-a", "mod-b"]
        );
    }

    #[test]
    fn install_manifest_status_request_accepts_optional_game_id_for_recovery_scan() {
        let value = json!({
            "gameId": "mhw",
            "profileId": "default",
            "modIds": ["mod-a"]
        });

        let request: crate::dto::InstallManifestStatusRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let (game_id, app_request) =
            install_manifest_status_request_from_dto(request).expect("valid ids should map");

        assert_eq!(game_id.expect("game id").as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(app_request.mod_ids[0].as_str(), "mod-a");
    }

    #[test]
    fn recovery_summary_maps_rollback_required_to_manifest_status_summary() {
        let summary = InstallRecoverySummary {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: InstallRecoveryStatus::RollbackRequired,
            managed_file_count: 2,
            backup_count: 1,
            issue_count: 0,
            issues: Vec::new(),
        };

        let manifest_summary = recovery_summary_to_manifest_status_summary(summary);

        assert_eq!(
            manifest_summary.status,
            hmm_app::InstallManifestStatus::RollbackRequired
        );
        assert_eq!(manifest_summary.managed_file_count, 2);
        assert_eq!(manifest_summary.backup_count, 1);
    }

    #[test]
    fn install_recovery_scan_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "profileId": "default",
            "modIds": ["mod-a", "mod-b"]
        });

        let request: crate::dto::InstallRecoveryScanRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let (game_id, app_request) =
            install_recovery_scan_request_from_dto(request).expect("valid ids should map");

        assert_eq!(game_id.as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(
            app_request
                .mod_ids
                .iter()
                .map(|mod_id| mod_id.as_str())
                .collect::<Vec<_>>(),
            vec!["mod-a", "mod-b"]
        );
    }

    #[test]
    fn install_recovery_scan_request_allows_empty_mod_ids_for_profile_scan() {
        let value = json!({
            "gameId": "mhw",
            "profileId": "default",
            "modIds": []
        });

        let request: crate::dto::InstallRecoveryScanRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let (game_id, app_request) =
            install_recovery_scan_request_from_dto(request).expect("empty mod ids should map");

        assert_eq!(game_id.as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert!(app_request.mod_ids.is_empty());
    }

    #[test]
    fn install_recovery_action_preview_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "profileId": "default",
            "modId": "mod-a",
            "actionKind": "reconcile_reinstall"
        });

        let request: crate::dto::InstallRecoveryActionPreviewRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let (game_id, app_request) = install_recovery_action_preview_request_from_dto(request)
            .expect("valid ids should map");

        assert_eq!(game_id.as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(app_request.mod_id.as_str(), "mod-a");
        assert_eq!(
            app_request.action_kind,
            hmm_app::InstallRecoveryActionKind::ReconcileReinstall
        );
    }

    #[test]
    fn start_recovery_action_task_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "profileId": "default",
            "modId": "mod-a",
            "actionKind": "rollback_install"
        });

        let request: StartRecoveryActionTaskRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let app_request =
            start_recovery_action_task_request_from_dto(request).expect("valid ids should map");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(app_request.mod_id.as_str(), "mod-a");
        assert_eq!(
            app_request.action_kind,
            hmm_app::InstallRecoveryActionKind::RollbackInstall
        );
    }

    #[test]
    fn queued_install_event_uses_registered_phase() {
        let task = TaskStarted {
            task_id: "install-123".to_owned(),
            kind: hmm_app::TaskKind::Install,
            status: hmm_app::TaskStatus::Queued,
        };

        let dto: TaskProgressEventDto = queued_event_for_started_install_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize event");

        assert_eq!(value["taskId"], "install-123");
        assert_eq!(value["kind"], "install");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], INSTALL_QUEUED_PHASE);
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn queued_uninstall_event_uses_registered_phase() {
        let task = TaskStarted {
            task_id: "install-123".to_owned(),
            kind: hmm_app::TaskKind::Install,
            status: hmm_app::TaskStatus::Queued,
        };

        let dto: TaskProgressEventDto = queued_event_for_started_uninstall_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize event");

        assert_eq!(value["taskId"], "install-123");
        assert_eq!(value["kind"], "install");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], INSTALL_UNINSTALL_QUEUED_PHASE);
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn queued_recovery_action_event_uses_registered_phase() {
        let task = TaskStarted {
            task_id: "install-123".to_owned(),
            kind: hmm_app::TaskKind::Install,
            status: hmm_app::TaskStatus::Queued,
        };

        let dto: TaskProgressEventDto = queued_event_for_started_recovery_action_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize event");

        assert_eq!(value["taskId"], "install-123");
        assert_eq!(value["kind"], "install");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], INSTALL_RECOVERY_QUEUED_PHASE);
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn install_plan_preview_serializes_actions_and_conflicts() {
        let action_target =
            InstallTargetPath::parse("content/models/player.mod3", ["content"]).expect("target");
        let conflict_target =
            InstallTargetPath::parse("content/models/weapon.mod3", ["content"]).expect("target");
        let provider_a = provider("mod-a", "file-a", action_target.clone(), "base", 10);
        let provider_b = provider("mod-b", "file-b", conflict_target.clone(), "base", 0);
        let provider_c = provider("mod-c", "file-c", conflict_target.clone(), "base", 0);
        let plan = InstallPlan {
            actions: vec![InstallAction {
                target_path: action_target,
                provider: provider_a,
            }],
            conflicts: vec![InstallConflict {
                target_path: conflict_target,
                providers: vec![provider_b, provider_c],
            }],
            replacement_bindings: Vec::new(),
        };

        let dto: InstallPlanPreviewDto = plan.into();
        let value: Value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["hasBlockingConflicts"], true);
        assert_eq!(
            value["actions"][0]["targetPath"],
            "content/models/player.mod3"
        );
        assert_eq!(value["actions"][0]["modId"], "mod-a");
        assert_eq!(value["actions"][0]["packageFileId"], "file-a");
        assert_eq!(value["actions"][0]["layerName"], "base");
        assert_eq!(value["actions"][0]["layerPriority"], 10);
        assert_eq!(
            value["conflicts"][0]["targetPath"],
            "content/models/weapon.mod3"
        );
        assert_eq!(value["conflicts"][0]["providers"][0]["modId"], "mod-b");
        assert_eq!(
            value["conflicts"][0]["providers"][1]["packageFileId"],
            "file-c"
        );
    }

    #[test]
    fn invalid_target_path_error_uses_stable_code_without_paths() {
        let error = install_planning_error_to_command_error(
            hmm_app::InstallPlanningError::InvalidTargetPath {
                package_file_id: PackageFileId::new("file-a"),
                source: InstallTargetPathError::ParentTraversal,
            },
        );

        assert_eq!(error.code, "install_target_path_parent_traversal");
        assert!(!error.message.contains("../"));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn install_recovery_unavailable_error_uses_stable_code_without_paths() {
        let error = install_recovery_scan_error_to_command_error(
            hmm_app::InstallRecoveryScanError::ManifestUnavailable,
        );

        assert_eq!(error.code, "install_recovery_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
        assert!(!error.message.contains("manifest"));
    }

    #[test]
    fn install_recovery_action_preview_unavailable_error_uses_stable_code_without_paths() {
        let error = install_recovery_action_preview_error_to_command_error(
            hmm_app::InstallRecoveryActionPreviewError::PreviewUnavailable,
        );

        assert_eq!(error.code, "install_recovery_action_preview_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
        assert!(!error.message.contains("manifest"));
        assert!(!error.message.contains("backup"));
    }

    #[test]
    fn file_input_maps_to_app_request_file() {
        let input = PreviewInstallPlanFileInputDto {
            mod_id: "mod-a".to_owned(),
            package_file_id: "file-a".to_owned(),
            target_path: "content/models/player.mod3".to_owned(),
            layer_name: "base".to_owned(),
            layer_priority: 10,
        };

        let file = install_plan_file_from_dto(input);

        assert_eq!(file.mod_id.as_str(), "mod-a");
        assert_eq!(file.package_file_id.as_str(), "file-a");
        assert_eq!(file.target_path, "content/models/player.mod3");
        assert_eq!(file.layer.name, "base");
        assert_eq!(file.layer.priority, 10);
    }

    fn provider(
        mod_id: &str,
        package_file_id: &str,
        target_path: InstallTargetPath,
        layer_name: &str,
        layer_priority: i32,
    ) -> InstallFileProvider {
        InstallFileProvider::new(
            ModId::new(mod_id),
            PackageFileId::new(package_file_id),
            target_path,
            FileLayer::new(layer_name, layer_priority),
        )
    }
}
