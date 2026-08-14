use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::save_restore_dto::{
    PreviewSaveRestoreRequestDto, SaveRestorePreviewDto, StartSaveRestoreTaskRequestDto,
};
use crate::state::AppState;
use crate::task_events::{emit_task_progress, TauriTaskProgressObserver};
use hmm_app::{
    PreviewSaveRestoreRequest, SaveRestorePreviewError, StartSaveRestoreRequest, TaskProgressEvent,
};
use hmm_core::{GameId, ProfileId};
use std::sync::Arc;
use tauri::{AppHandle, State};

const SAVE_RESTORE_QUEUED_PHASE: &str = "save_restore.queued";

#[tauri::command]
pub async fn preview_save_restore(
    request: PreviewSaveRestoreRequestDto,
    state: State<'_, AppState>,
) -> Result<SaveRestorePreviewDto, CommandErrorDto> {
    let request = preview_request_from_dto(request)?;
    let service = Arc::clone(&state.save_restore);
    tauri::async_runtime::spawn_blocking(move || service.preview(request))
        .await
        .map_err(|_| save_restore_command_error("save_restore_preview_unavailable"))?
        .and_then(|preview| {
            SaveRestorePreviewDto::try_from(preview)
                .map_err(|_| SaveRestorePreviewError::TokenIssueFailed)
        })
        .map_err(save_restore_error_to_command_error)
}

#[tauri::command]
pub fn start_save_restore_task(
    request: StartSaveRestoreTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = state
        .save_restore_tasks
        .start_save_restore_task(&request)
        .map_err(CommandErrorDto::from_task_manager_error)?;
    if let Err(error) = emit_task_progress(
        &app_handle,
        TaskProgressEvent::new(
            task.task_id.clone(),
            task.kind,
            task.status,
            SAVE_RESTORE_QUEUED_PHASE,
        ),
    ) {
        let _ = state
            .save_restore_tasks
            .abort_queued_save_restore_task(&request, &task.task_id);
        return Err(error);
    }
    spawn_save_restore_runner(
        Arc::clone(&state.save_restore_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );
    Ok(task.into())
}

fn spawn_save_restore_runner(
    runner: Arc<hmm_app::SaveRestoreTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartSaveRestoreRequest,
) {
    std::thread::spawn(move || {
        let observer = TauriTaskProgressObserver::new(&app_handle);
        let _ = runner.run_save_restore_task_with_observer(&task_id, request, &observer);
    });
}

fn preview_request_from_dto(
    request: PreviewSaveRestoreRequestDto,
) -> Result<PreviewSaveRestoreRequest, CommandErrorDto> {
    Ok(PreviewSaveRestoreRequest {
        game_id: parse_game_id(request.game_id)?,
        profile_id: parse_profile_id(request.profile_id)?,
        backup_id: normalize_backup_id(request.backup_id)?,
    })
}

fn start_request_from_dto(
    request: StartSaveRestoreTaskRequestDto,
) -> Result<StartSaveRestoreRequest, CommandErrorDto> {
    Ok(StartSaveRestoreRequest {
        game_id: parse_game_id(request.game_id)?,
        profile_id: parse_profile_id(request.profile_id)?,
        backup_id: normalize_backup_id(request.backup_id)?,
        preview_token: normalize_preview_token(request.preview_token)?,
        confirmed: request.confirmed,
        confirmed_without_pre_restore: request.confirmed_without_pre_restore,
    })
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|_| save_restore_command_error("game_id_invalid"))
}

fn parse_profile_id(value: String) -> Result<ProfileId, CommandErrorDto> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(save_restore_command_error("profile_id_invalid"));
    }
    Ok(ProfileId::new(value))
}

fn normalize_backup_id(value: String) -> Result<String, CommandErrorDto> {
    let value = value.trim();
    let parts = value.split(':').collect::<Vec<_>>();
    let valid = !value.is_empty()
        && value.len() <= 256
        && if parts.len() == 1 {
            is_backup_id_component(parts[0])
        } else {
            (4..=5).contains(&parts.len())
                && parts.iter().all(|part| {
                    !part.is_empty()
                        && part.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                        })
                })
        };
    if !valid {
        return Err(save_restore_command_error("save_restore_backup_id_invalid"));
    }
    Ok(value.to_owned())
}

fn is_backup_id_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn normalize_preview_token(value: String) -> Result<String, CommandErrorDto> {
    normalize_opaque_id(value, "save_restore_preview_token_invalid", 512)
}

fn normalize_opaque_id(
    value: String,
    error_code: &str,
    max_len: usize,
) -> Result<String, CommandErrorDto> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > max_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(save_restore_command_error(error_code));
    }
    Ok(value.to_owned())
}

fn save_restore_error_to_command_error(error: SaveRestorePreviewError) -> CommandErrorDto {
    save_restore_command_error(error.code())
}

fn save_restore_command_error(code: &str) -> CommandErrorDto {
    CommandErrorDto {
        code: code.to_owned(),
        message: "save restore operation failed".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_mapping_accepts_only_short_ids_and_token() {
        let request: StartSaveRestoreTaskRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "default",
            "backupId": "mhw:profile-default:20260704-221530:manual",
            "previewToken": "hmm-save-restore-v1.1.2.signature",
            "confirmed": true,
            "confirmedWithoutPreRestore": false
        }))
        .expect("request dto");
        let request = start_request_from_dto(request).expect("safe request");
        assert_eq!(
            request.backup_id,
            "mhw:profile-default:20260704-221530:manual"
        );
        assert!(!request.preview_token.contains('/') && !request.preview_token.contains('\\'));
    }

    #[test]
    fn request_mapping_rejects_path_shaped_backup_id() {
        for value in [
            "C:/Users/Alice/save.zip",
            "C:save.zip",
            "backup:1",
            "mhw:profile-default:20260704-221530",
            "mhw::20260704-221530:manual",
        ] {
            let error = normalize_backup_id(value.to_owned()).expect_err("paths are not accepted");
            assert_eq!(error.code, "save_restore_backup_id_invalid");
            assert!(!error.message.contains(value));
        }
    }

    #[test]
    fn request_mapping_accepts_sequence_suffix_from_persisted_backup_ids() {
        let request =
            normalize_backup_id("mhw:profile-default:20260704-221530:manual:02".to_owned())
                .expect("writer sequence suffix is accepted");
        assert_eq!(request, "mhw:profile-default:20260704-221530:manual:02");
    }

    #[test]
    fn request_mapping_rejects_colon_shaped_preview_token() {
        let error = normalize_preview_token("hmm-save-restore-v1:token".to_owned())
            .expect_err("colon-shaped token must be rejected");
        assert_eq!(error.code, "save_restore_preview_token_invalid");
        assert!(!error.message.contains("hmm-save-restore-v1:token"));
    }

    #[test]
    fn request_mapping_rejects_path_shaped_profile_id() {
        let error = parse_profile_id("C:/Users/Alice".to_owned())
            .expect_err("profile paths are not accepted");
        assert_eq!(error.code, "profile_id_invalid");
        assert!(!error.message.contains("C:/Users"));
    }

    #[test]
    fn queued_phase_and_preparing_phase_are_distinct() {
        assert_eq!(SAVE_RESTORE_QUEUED_PHASE, "save_restore.queued");
        assert_eq!(
            hmm_app::SAVE_RESTORE_PREPARING_PHASE,
            "save_restore.preparing"
        );
    }
}
