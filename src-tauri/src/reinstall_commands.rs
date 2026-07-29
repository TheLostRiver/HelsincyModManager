use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::reinstall_dto::{
    ModRevisionListDto, PreviewReinstallPlanRequestDto, ReinstallPlanPreviewDto,
    StartReinstallTaskRequestDto,
};
use crate::state::{AppState, ConfiguredReinstallExecutor};
use crate::task_events::{emit_task_progress, INSTALL_REINSTALL_QUEUED_PHASE};
use hmm_app::{
    ReinstallPreviewError, ReinstallPreviewRequest, StartReinstallTaskRequest, TaskProgressEvent,
    TaskStarted,
};
use hmm_core::{FileLayer, GameId, ModId, ModRevisionId, ProfileId};
use std::sync::Arc;
use tauri::{AppHandle, State};

const PLAN_TOKEN_PREFIX: &str = "reinstall-preview-v1:";
const PLAN_TOKEN_DIGEST_LENGTH: usize = 64;

#[tauri::command]
pub fn get_mod_revisions(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<ModRevisionListDto, CommandErrorDto> {
    let mod_id = ModId::new(parse_non_empty_id(
        mod_id,
        "mod_id_empty",
        "mod id cannot be empty",
    )?);
    let revisions = state
        .mod_library
        .get_mod_revisions(&mod_id)
        .map_err(|_| mod_revisions_unavailable_error())?
        .ok_or_else(mod_revisions_not_found_error)?;

    Ok(revisions.into())
}

#[tauri::command]
pub fn preview_reinstall_plan(
    request: PreviewReinstallPlanRequestDto,
    state: State<'_, AppState>,
) -> Result<ReinstallPlanPreviewDto, CommandErrorDto> {
    let preview = state
        .reinstall_executor
        .preview(preview_request_from_dto(request)?)
        .map_err(preview_error_to_command_error)?;
    ReinstallPlanPreviewDto::try_from(preview).map_err(|_| reinstall_preview_invariant_error())
}

#[tauri::command]
pub fn start_reinstall_task(
    request: StartReinstallTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = state
        .reinstall_tasks
        .start_reinstall_task(request)
        .map_err(|_| reinstall_start_unavailable_error())?;

    let _ = emit_task_progress(&app_handle, queued_event_for_started_reinstall_task(&task));
    spawn_reinstall_runner(
        Arc::clone(&state.reinstall_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );

    Ok(task.into())
}

fn spawn_reinstall_runner(
    runner: Arc<hmm_app::ReinstallTaskRunner<ConfiguredReinstallExecutor>>,
    app_handle: AppHandle,
    task_id: String,
    request: StartReinstallTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_reinstall_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };
        for event in events {
            let _ = emit_task_progress(&app_handle, event);
        }
    });
}

fn queued_event_for_started_reinstall_task(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        INSTALL_REINSTALL_QUEUED_PHASE,
    )
}

fn preview_request_from_dto(
    request: PreviewReinstallPlanRequestDto,
) -> Result<ReinstallPreviewRequest, CommandErrorDto> {
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
    let candidate_revision_id = parse_non_empty_id(
        request.candidate_revision_id,
        "candidate_revision_id_empty",
        "candidate revision id cannot be empty",
    )?;
    let layer_name = parse_non_empty_id(
        request.layer.name,
        "layer_name_empty",
        "layer name cannot be empty",
    )?;

    Ok(ReinstallPreviewRequest {
        game_id,
        profile_id: ProfileId::new(profile_id),
        mod_id: ModId::new(mod_id),
        candidate_revision_id: ModRevisionId::new(candidate_revision_id),
        layer: FileLayer::new(layer_name, request.layer.priority),
    })
}

fn start_request_from_dto(
    request: StartReinstallTaskRequestDto,
) -> Result<StartReinstallTaskRequest, CommandErrorDto> {
    let plan_token = parse_plan_token(request.plan_token)?;
    let preview = preview_request_from_dto(PreviewReinstallPlanRequestDto {
        game_id: request.game_id,
        profile_id: request.profile_id,
        mod_id: request.mod_id,
        candidate_revision_id: request.candidate_revision_id,
        layer: request.layer,
    })?;
    Ok(StartReinstallTaskRequest {
        game_id: preview.game_id,
        profile_id: preview.profile_id,
        mod_id: preview.mod_id,
        candidate_revision_id: preview.candidate_revision_id,
        layer: preview.layer,
        plan_token,
    })
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

pub(crate) fn parse_plan_token(value: String) -> Result<String, CommandErrorDto> {
    let value = value.trim();
    let digest = value.strip_prefix(PLAN_TOKEN_PREFIX);
    if digest.is_none_or(|digest| {
        digest.len() != PLAN_TOKEN_DIGEST_LENGTH
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(CommandErrorDto {
            code: "plan_token_invalid".to_owned(),
            message: "reinstall plan token is invalid".to_owned(),
        });
    }
    Ok(value.to_owned())
}

pub(crate) fn preview_error_to_command_error(error: ReinstallPreviewError) -> CommandErrorDto {
    let (code, message) = match error {
        ReinstallPreviewError::CatalogUnavailable => (
            "reinstall_catalog_unavailable",
            "reinstall catalog is unavailable",
        ),
        ReinstallPreviewError::ManifestUnavailable => (
            "reinstall_manifest_unavailable",
            "reinstall manifest is unavailable",
        ),
        ReinstallPreviewError::RecoveryUnavailable => (
            "reinstall_recovery_unavailable",
            "reinstall recovery state is unavailable",
        ),
        ReinstallPreviewError::CandidatePlanUnavailable => (
            "reinstall_candidate_plan_unavailable",
            "reinstall candidate plan is unavailable",
        ),
    };
    CommandErrorDto {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn reinstall_preview_invariant_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "reinstall_preview_unavailable".to_owned(),
        message: "reinstall preview is unavailable".to_owned(),
    }
}

fn reinstall_start_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "reinstall_start_unavailable".to_owned(),
        message: "reinstall task cannot be started".to_owned(),
    }
}

fn mod_revisions_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_revisions_unavailable".to_owned(),
        message: "Mod revisions are unavailable".to_owned(),
    }
}

fn mod_revisions_not_found_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_revisions_not_found".to_owned(),
        message: "Mod revisions were not found".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TaskProgressEventDto;
    use crate::reinstall_dto::{PreviewReinstallPlanRequestDto, StartReinstallTaskRequestDto};
    use hmm_app::{TaskKind, TaskProgressEvent, TaskStatus};
    use serde_json::{json, Value};

    #[test]
    fn preview_parser_maps_only_controlled_ids_and_layer() {
        let request: PreviewReinstallPlanRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "default",
            "modId": "mod-a",
            "candidateRevisionId": "revision-v2",
            "layer": { "name": "base", "priority": 4 }
        }))
        .expect("deserialize request");

        let request = preview_request_from_dto(request).expect("parse request");

        assert_eq!(request.game_id.as_str(), "mhw");
        assert_eq!(request.profile_id.as_str(), "default");
        assert_eq!(request.mod_id.as_str(), "mod-a");
        assert_eq!(request.candidate_revision_id.as_str(), "revision-v2");
        assert_eq!(request.layer.name, "base");
        assert_eq!(request.layer.priority, 4);
    }

    #[test]
    fn start_parser_rejects_empty_and_malformed_plan_tokens() {
        for plan_token in ["", "opaque", "reinstall-preview-v1:abc"] {
            let request: StartReinstallTaskRequestDto = serde_json::from_value(json!({
                "gameId": "mhw",
                "profileId": "default",
                "modId": "mod-a",
                "candidateRevisionId": "revision-v2",
                "layer": { "name": "base", "priority": 0 },
                "planToken": plan_token
            }))
            .expect("deserialize request");

            let error = start_request_from_dto(request).expect_err("token must be rejected");
            assert_eq!(error.code, "plan_token_invalid");
        }
    }

    #[test]
    fn invalid_ids_use_stable_sanitized_errors() {
        let request: PreviewReinstallPlanRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": " ",
            "modId": "mod-a",
            "candidateRevisionId": "revision-v2",
            "layer": { "name": "base", "priority": 0 }
        }))
        .expect("deserialize request");

        let error = preview_request_from_dto(request).expect_err("empty profile id");
        assert_eq!(error.code, "profile_id_empty");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn preview_unavailable_errors_use_stable_sanitized_codes() {
        let cases = [
            (
                ReinstallPreviewError::CatalogUnavailable,
                "reinstall_catalog_unavailable",
            ),
            (
                ReinstallPreviewError::ManifestUnavailable,
                "reinstall_manifest_unavailable",
            ),
            (
                ReinstallPreviewError::RecoveryUnavailable,
                "reinstall_recovery_unavailable",
            ),
            (
                ReinstallPreviewError::CandidatePlanUnavailable,
                "reinstall_candidate_plan_unavailable",
            ),
        ];

        for (source, expected_code) in cases {
            let error = preview_error_to_command_error(source);
            assert_eq!(error.code, expected_code);
            assert!(!error.message.contains(':'));
            assert!(!error.message.contains('\\'));
        }
    }

    #[test]
    fn queued_event_uses_reinstall_phase_and_existing_install_task_shape() {
        let task = TaskStarted {
            task_id: "install-123".to_owned(),
            kind: TaskKind::Install,
            status: TaskStatus::Queued,
        };

        let dto: TaskProgressEventDto = queued_event_for_started_reinstall_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize queued event");

        assert_eq!(
            value,
            json!({
                "taskId": "install-123",
                "kind": "install",
                "status": "queued",
                "phase": "install.reinstall.queued",
                "current": null,
                "total": null,
                "message": null,
                "error": null,
                "resultRef": null
            })
        );
    }

    #[test]
    fn post_commit_task_error_remains_failure_not_rolled_back() {
        let mut event = TaskProgressEvent::new(
            "install-123",
            TaskKind::Install,
            TaskStatus::Failed,
            "install.reinstall.failed",
        );
        event.error = Some("install_reinstall_failed:post_commit".to_owned());

        let value = serde_json::to_value(TaskProgressEventDto::from(event))
            .expect("serialize failed event");

        assert_eq!(value["error"], "install_reinstall_failed:post_commit");
        assert_eq!(value["status"], "failed");
        assert_ne!(value["error"], "rolled_back");
    }
}
