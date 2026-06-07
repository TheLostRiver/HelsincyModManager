use crate::dto::{
    candidate_scan_to_dto, status_to_dto, validation_to_dto, CommandErrorDto, GameCandidateScanDto,
    GameDirectoryValidationDto, GameSetupStatusDto,
};
use crate::state::AppState;
use hmm_core::GameId;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn get_game_setup_status(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<GameSetupStatusDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;

    state
        .game_setup
        .get_status(game_id)
        .map(status_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn validate_game_directory(
    game_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<GameDirectoryValidationDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let directory = parse_directory(directory)?;

    state
        .game_setup
        .validate_directory(game_id, directory)
        .map(validation_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn save_game_directory(
    game_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<GameSetupStatusDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let directory = parse_directory(directory)?;

    state
        .game_setup
        .save_game_directory(game_id, directory)
        .map(status_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn scan_game_candidates(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<GameCandidateScanDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;

    state
        .game_setup
        .scan_candidates(game_id)
        .map(candidate_scan_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|error| CommandErrorDto {
        code: "unsupported_game".to_owned(),
        message: error.to_string(),
    })
}

fn parse_directory(value: String) -> Result<PathBuf, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "directory_not_found".to_owned(),
            message: "directory cannot be empty".to_owned(),
        });
    }

    let directory = PathBuf::from(trimmed);

    if !directory.is_absolute() {
        return Err(CommandErrorDto {
            code: "directory_not_absolute".to_owned(),
            message: "directory must be an absolute path".to_owned(),
        });
    }

    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_directory_rejects_relative_paths() {
        let error = parse_directory("Monster Hunter World".to_owned())
            .expect_err("relative paths must be rejected");

        assert_eq!(error.code, "directory_not_absolute");
    }
}
