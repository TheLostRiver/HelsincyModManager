use crate::dto::CommandErrorDto;
use crate::save_backup_center_dto::{
    QuerySaveBackupCenterRequestDto, RunSaveBackupRetentionRequestDto, SaveBackupCenterPageDto,
    SaveBackupRetentionReportDto, UpdateSaveBackupNoteRequestDto, UpdateSaveBackupNoteResultDto,
};
use crate::state::AppState;
use hmm_app::{SaveBackupCenterError, SaveBackupCenterQuery};
use hmm_core::{GameId, ProfileId};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn query_save_backup_center(
    request: QuerySaveBackupCenterRequestDto,
    state: State<'_, AppState>,
) -> Result<SaveBackupCenterPageDto, CommandErrorDto> {
    let query = SaveBackupCenterQuery {
        game_id: parse_game_id(request.game_id)?,
        profile_id: request.profile_id.map(parse_profile_id).transpose()?,
        trigger: request.trigger.map(Into::into),
        status: request.status.map(Into::into),
        search: request.search,
        offset: request.offset,
        limit: request.limit,
    };
    let service = Arc::clone(&state.save_backup_center);
    tauri::async_runtime::spawn_blocking(move || service.query(query))
        .await
        .map_err(|_| unavailable_error())?
        .map(Into::into)
        .map_err(center_error)
}

#[tauri::command]
pub async fn update_save_backup_note(
    request: UpdateSaveBackupNoteRequestDto,
    state: State<'_, AppState>,
) -> Result<UpdateSaveBackupNoteResultDto, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let profile_id = parse_profile_id(request.profile_id)?;
    let backup_id = parse_backup_id(request.backup_id)?;
    let service = Arc::clone(&state.save_backup_center);
    let note = tauri::async_runtime::spawn_blocking(move || {
        service.update_note(&game_id, &profile_id, &backup_id, request.note)
    })
    .await
    .map_err(|_| unavailable_error())?
    .map_err(center_error)?;
    Ok(UpdateSaveBackupNoteResultDto { note })
}

#[tauri::command]
pub async fn run_save_backup_retention(
    request: RunSaveBackupRetentionRequestDto,
    state: State<'_, AppState>,
) -> Result<SaveBackupRetentionReportDto, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let profile_id = parse_profile_id(request.profile_id)?;
    let service = Arc::clone(&state.save_backup_center);
    tauri::async_runtime::spawn_blocking(move || service.run_retention(&game_id, &profile_id))
        .await
        .map_err(|_| unavailable_error())?
        .map(Into::into)
        .map_err(center_error)
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })
}

fn parse_profile_id(value: String) -> Result<ProfileId, CommandErrorDto> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CommandErrorDto {
            code: "profile_id_invalid".to_owned(),
            message: "profile id is invalid".to_owned(),
        });
    }
    Ok(ProfileId::new(value.to_owned()))
}

fn parse_backup_id(value: String) -> Result<String, CommandErrorDto> {
    let value = value.trim();
    let parts = value.split(':').collect::<Vec<_>>();
    let valid = !value.is_empty()
        && value.len() <= 256
        && if parts.len() == 1 {
            backup_id_component_is_valid(parts[0])
        } else {
            (4..=5).contains(&parts.len())
                && parts.iter().all(|part| backup_id_component_is_valid(part))
        };
    if !valid {
        return Err(CommandErrorDto {
            code: "backup_id_invalid".to_owned(),
            message: "backup id is invalid".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn backup_id_component_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn center_error(error: SaveBackupCenterError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "save backup center operation failed".to_owned(),
    }
}

fn unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "save_backup_center_unavailable".to_owned(),
        message: "save backup center is unavailable".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_id_parsers_reject_path_like_values() {
        assert_eq!(
            parse_profile_id("C:/Users/Player".to_owned())
                .expect_err("path-like profile id")
                .code,
            "profile_id_invalid"
        );
        assert_eq!(
            parse_backup_id("..\\archive.zip".to_owned())
                .expect_err("path-like backup id")
                .code,
            "backup_id_invalid"
        );
        assert!(parse_backup_id("mhw:default:20260815-120000:manual".to_owned()).is_ok());
    }

    #[test]
    fn center_errors_map_to_stable_codes_without_backend_details() {
        let cases = [
            (
                SaveBackupCenterError::QueryInvalid,
                "save_backup_center_query_invalid",
            ),
            (
                SaveBackupCenterError::RepositoryUnavailable,
                "save_backup_center_unavailable",
            ),
            (
                SaveBackupCenterError::ProfileMissing,
                "save_backup_center_profile_missing",
            ),
            (
                SaveBackupCenterError::NoteInvalid,
                "save_backup_note_invalid",
            ),
            (
                SaveBackupCenterError::BackupMissing,
                "save_backup_center_backup_missing",
            ),
            (
                SaveBackupCenterError::TaskConflict,
                "save_backup_task_conflict",
            ),
            (
                SaveBackupCenterError::RetentionFailed,
                "save_backup_retention_failed",
            ),
        ];

        for (error, expected_code) in cases {
            let dto = center_error(error);
            assert_eq!(dto.code, expected_code);
            assert_eq!(dto.message, "save backup center operation failed");
            assert!(!dto.message.contains(':'));
            assert!(!dto.message.contains('\\'));
        }
    }
}
