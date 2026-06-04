use crate::dto::{
    status_to_dto, validation_to_dto, CommandErrorDto, GameDirectoryValidationDto,
    GameSetupStatusDto,
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
    let service = lock_service(&state)?;

    service
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
    let service = lock_service(&state)?;

    service
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
    let service = lock_service(&state)?;

    service
        .save_game_directory(game_id, directory)
        .map(status_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn scan_game_candidates(game_id: String, state: State<'_, AppState>) -> Result<(), CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let service = lock_service(&state)?;

    service
        .scan_candidates(game_id)
        .map_err(CommandErrorDto::from_service_error)
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|error| CommandErrorDto {
        code: "unsupported_game".to_owned(),
        message: error.to_string(),
    })
}

fn parse_directory(value: String) -> Result<PathBuf, CommandErrorDto> {
    if value.trim().is_empty() {
        return Err(CommandErrorDto {
            code: "directory_not_found".to_owned(),
            message: "directory cannot be empty".to_owned(),
        });
    }

    Ok(PathBuf::from(value))
}

fn lock_service<'a>(
    state: &'a State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, hmm_app::GameSetupService>, CommandErrorDto> {
    state.game_setup.lock().map_err(|_| CommandErrorDto {
        code: "unknown".to_owned(),
        message: "game setup state lock failed".to_owned(),
    })
}
