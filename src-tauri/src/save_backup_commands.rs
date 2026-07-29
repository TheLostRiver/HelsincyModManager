use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::save_backup_dto::{
    CheckAutoSaveBackupRequestDto, GetSaveBackupBackgroundStatusRequestDto,
    ListSaveBackupsRequestDto, ProfileAutoSaveBackupCheckDto, SaveBackupBackgroundControlStatusDto,
    SaveBackupBackgroundStatusDto, SaveBackupSummaryDto, StartSaveBackupTaskRequestDto,
};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{
    SaveBackupAutoCheckRequest, SaveBackupAutoSchedulerError, SaveBackupBackgroundControlStatus,
    SaveBackupBackgroundService, SaveBackupBackgroundServiceError, SaveBackupError,
    StartSaveBackupTaskRequest, TaskProgressEvent, TaskStarted,
};
use hmm_core::{GameId, ProfileId, SaveBackupTrigger};
use std::sync::Arc;
use tauri::{AppHandle, State};

const SAVE_BACKUP_QUEUED_PHASE: &str = "save_backup.queued";
type BackgroundControlOperation =
    fn(
        &SaveBackupBackgroundService,
    ) -> Result<SaveBackupBackgroundControlStatus, SaveBackupBackgroundServiceError>;

#[tauri::command]
pub fn start_save_backup_task(
    request: StartSaveBackupTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_save_backup_task_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = state
        .save_backup_tasks
        .start_save_backup_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)?;

    let _ = emit_task_progress(
        &app_handle,
        queued_event_for_started_save_backup_task(&task),
    );
    spawn_save_backup_runner(
        Arc::clone(&state.save_backup_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );

    Ok(task.into())
}

#[tauri::command]
pub fn list_save_backups(
    request: ListSaveBackupsRequestDto,
    state: State<'_, AppState>,
) -> Result<Vec<SaveBackupSummaryDto>, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let profile_id = parse_profile_id(request.profile_id)?;
    let backups = state
        .save_backups
        .list_backups(
            &game_id,
            &profile_id,
            request.limit.map(|value| value as usize),
        )
        .map_err(save_backup_error_to_command_error)?;

    Ok(backups.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub fn check_auto_save_backup(
    request: CheckAutoSaveBackupRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ProfileAutoSaveBackupCheckDto, CommandErrorDto> {
    let request = check_auto_save_backup_request_from_dto(request)?;
    let mut result = state
        .save_backup_auto_scheduler
        .check_profile(request)
        .map_err(auto_save_backup_error_to_command_error)?;
    let due_task = result.due_task.take();
    let started_task = if let Some(request) = due_task {
        let runner_request = request.clone();
        let task = state
            .save_backup_tasks
            .start_save_backup_task(request)
            .map_err(CommandErrorDto::from_task_manager_error)?;

        let _ = emit_task_progress(
            &app_handle,
            queued_event_for_started_save_backup_task(&task),
        );
        spawn_save_backup_runner(
            Arc::clone(&state.save_backup_task_runner),
            app_handle,
            task.task_id.clone(),
            runner_request,
        );

        Some(TaskStartedDto::from(task))
    } else {
        None
    };

    Ok(ProfileAutoSaveBackupCheckDto::from_result(
        result,
        started_task,
    ))
}

#[tauri::command]
pub async fn get_save_backup_background_status(
    request: GetSaveBackupBackgroundStatusRequestDto,
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundStatusDto, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let profile_id = parse_profile_id(request.profile_id)?;
    let service = Arc::clone(&state.save_backup_background);
    let query_game_id = game_id.clone();
    let query_profile_id = profile_id.clone();
    let background = tauri::async_runtime::spawn_blocking(move || {
        service.status(&query_game_id, &query_profile_id)
    })
    .await
    .map_err(|_| save_backup_background_status_unavailable_error())?
    .map_err(save_backup_background_error_to_command_error)?;

    Ok(SaveBackupBackgroundStatusDto::from_status(
        &game_id,
        &profile_id,
        background,
    ))
}

#[tauri::command]
pub async fn get_save_backup_background_control_status(
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    run_background_control(state, SaveBackupBackgroundService::control_status).await
}

#[tauri::command]
pub async fn enable_save_backup_background_protection(
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    run_background_control(state, SaveBackupBackgroundService::enable).await
}

#[tauri::command]
pub async fn disable_save_backup_background_protection(
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    run_background_control(state, SaveBackupBackgroundService::disable).await
}

async fn run_background_control(
    state: State<'_, AppState>,
    operation: BackgroundControlOperation,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    let service = Arc::clone(&state.save_backup_background);
    let status = tauri::async_runtime::spawn_blocking(move || operation(&service))
        .await
        .map_err(|_| save_backup_background_status_unavailable_error())?
        .map_err(save_backup_background_error_to_command_error)?;
    Ok(status.into())
}

fn spawn_save_backup_runner(
    runner: Arc<hmm_app::SaveBackupTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartSaveBackupTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_save_backup_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event);
        }
    });
}

fn queued_event_for_started_save_backup_task(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        SAVE_BACKUP_QUEUED_PHASE,
    )
}

fn start_save_backup_task_request_from_dto(
    request: StartSaveBackupTaskRequestDto,
) -> Result<StartSaveBackupTaskRequest, CommandErrorDto> {
    Ok(StartSaveBackupTaskRequest {
        game_id: parse_game_id(request.game_id)?,
        profile_id: parse_profile_id(request.profile_id)?,
        trigger: SaveBackupTrigger::Manual,
        note: normalize_note(request.note),
        scheduler_lease_owner: None,
    })
}

fn check_auto_save_backup_request_from_dto(
    request: CheckAutoSaveBackupRequestDto,
) -> Result<SaveBackupAutoCheckRequest, CommandErrorDto> {
    Ok(SaveBackupAutoCheckRequest {
        game_id: parse_game_id(request.game_id)?,
        profile_id: parse_profile_id(request.profile_id)?,
    })
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })
}

fn parse_profile_id(value: String) -> Result<ProfileId, CommandErrorDto> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "profile_id_empty".to_owned(),
            message: "profile id cannot be empty".to_owned(),
        });
    }

    Ok(ProfileId::new(trimmed.to_owned()))
}

fn normalize_note(note: Option<String>) -> Option<String> {
    note.map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn save_backup_error_to_command_error(error: SaveBackupError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "save backup operation failed".to_owned(),
    }
}

fn auto_save_backup_error_to_command_error(error: SaveBackupAutoSchedulerError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "auto save backup check failed".to_owned(),
    }
}

fn save_backup_background_error_to_command_error(
    error: SaveBackupBackgroundServiceError,
) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "save backup background status is unavailable".to_owned(),
    }
}

fn save_backup_background_status_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "save_backup_background_status_unavailable".to_owned(),
        message: "save backup background status is unavailable".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TaskProgressEventDto;
    use crate::save_backup_dto::{CheckAutoSaveBackupRequestDto, StartSaveBackupTaskRequestDto};
    use hmm_app::{SaveBackupBackgroundServiceError, SaveBackupError, TaskStarted};
    use serde_json::{json, Value};

    #[test]
    fn start_save_backup_task_request_maps_to_app_request_without_paths() {
        let request: StartSaveBackupTaskRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "default",
            "note": " before hunt "
        }))
        .expect("deserialize request");

        let app_request =
            start_save_backup_task_request_from_dto(request).expect("valid ids should map");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
        assert_eq!(app_request.trigger, hmm_core::SaveBackupTrigger::Manual);
        assert_eq!(app_request.note.as_deref(), Some("before hunt"));
    }

    #[test]
    fn check_auto_save_backup_request_maps_to_app_request_without_paths() {
        let request: CheckAutoSaveBackupRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "default"
        }))
        .expect("deserialize request");

        let app_request =
            check_auto_save_backup_request_from_dto(request).expect("valid ids should map");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.profile_id.as_str(), "default");
    }

    #[test]
    fn background_status_request_rejects_invalid_ids_with_stable_codes() {
        let request: GetSaveBackupBackgroundStatusRequestDto = serde_json::from_value(json!({
            "gameId": "unknown-game",
            "profileId": "default"
        }))
        .expect("deserialize request");
        let error = parse_game_id(request.game_id).expect_err("unknown game id is rejected");
        assert_eq!(error.code, "game_id_invalid");

        let error = parse_profile_id("   ".to_owned()).expect_err("blank profile id is rejected");
        assert_eq!(error.code, "profile_id_empty");
    }

    #[test]
    fn background_status_errors_map_to_stable_codes_without_details() {
        for (error, expected) in [
            (
                SaveBackupBackgroundServiceError::SchedulerStateUnavailable,
                "save_backup_scheduler_unavailable",
            ),
            (
                SaveBackupBackgroundServiceError::SettingsUnavailable,
                "save_backup_background_settings_unavailable",
            ),
            (
                SaveBackupBackgroundServiceError::ClockUnavailable,
                "save_backup_clock_unavailable",
            ),
            (
                SaveBackupBackgroundServiceError::AuditUnavailable,
                "save_backup_background_audit_unavailable",
            ),
        ] {
            let command_error = save_backup_background_error_to_command_error(error);
            assert_eq!(command_error.code, expected);
            assert_eq!(
                command_error.message,
                "save backup background status is unavailable"
            );
            assert!(!command_error.message.contains("C:/Users"));
            assert!(!command_error.message.contains("S-1-5-21"));
        }
    }

    #[test]
    fn background_control_join_failure_uses_stable_sanitized_error() {
        let error = save_backup_background_status_unavailable_error();

        assert_eq!(error.code, "save_backup_background_status_unavailable");
        assert_eq!(
            error.message,
            "save backup background status is unavailable"
        );
        assert!(!error.message.contains("C:/Users"));
        assert!(!error.message.contains("S-1-5-21"));
    }

    #[test]
    fn background_control_commands_are_exposed() {
        let _ = get_save_backup_background_control_status;
        let _ = enable_save_backup_background_protection;
        let _ = disable_save_backup_background_protection;
    }

    #[test]
    fn queued_save_backup_event_uses_registered_phase() {
        let task = TaskStarted {
            task_id: "save-backup-123".to_owned(),
            kind: hmm_app::TaskKind::SaveBackup,
            status: hmm_app::TaskStatus::Queued,
        };

        let dto: TaskProgressEventDto = queued_event_for_started_save_backup_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize event");

        assert_eq!(value["taskId"], "save-backup-123");
        assert_eq!(value["kind"], "save_backup");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], "save_backup.queued");
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn save_backup_error_maps_to_stable_command_error_without_paths() {
        let error = save_backup_error_to_command_error(SaveBackupError::SourceUnset);

        assert_eq!(error.code, "save_backup_source_unset");
        assert!(!error.message.contains("C:/"));
        assert!(!error.message.contains('\\'));
    }
}
