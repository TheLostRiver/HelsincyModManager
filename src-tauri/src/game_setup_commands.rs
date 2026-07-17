use crate::dto::{
    auto_detection_to_dto, candidate_scan_to_dto, prerequisite_report_to_dto, status_to_dto,
    validation_to_dto, CommandErrorDto, GameAutoDetectionDto, GameCandidateScanDto,
    GameDirectoryValidationDto, GamePrerequisiteReportDto, GameSetupStatusDto,
};
use crate::state::AppState;
use hmm_core::GameId;
use hmm_infra::{emit_safe_app_log, AppLogEvent};
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
pub fn get_game_prerequisite_status(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<GamePrerequisiteReportDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;

    state
        .game_setup
        .get_prerequisite_status(game_id)
        .map(prerequisite_report_to_dto)
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
    let log_game_id = game_id.as_str().to_owned();

    match state.game_setup.scan_candidates(game_id) {
        Ok(scan) => {
            let scan = candidate_scan_to_dto(scan);
            emit_safe_app_log(game_discovery_completed_event(
                &scan.game_id,
                "scan_candidates",
                "success",
                scan.candidates.len(),
            ));
            Ok(scan)
        }
        Err(error) => {
            let error = CommandErrorDto::from_service_error(error);
            emit_safe_app_log(game_discovery_failed_event(
                &log_game_id,
                "scan_candidates",
                &error.code,
            ));
            Err(error)
        }
    }
}

#[tauri::command]
pub fn auto_detect_game_directory(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<GameAutoDetectionDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let log_game_id = game_id.as_str().to_owned();

    match state.game_setup.auto_detect_game_directory(game_id) {
        Ok(detection) => {
            let detection = auto_detection_to_dto(detection);
            emit_safe_app_log(game_discovery_completed_event(
                &detection.game_id,
                "auto_detect",
                &detection.outcome,
                detection.candidate_count,
            ));
            Ok(detection)
        }
        Err(error) => {
            let error = CommandErrorDto::from_service_error(error);
            emit_safe_app_log(game_discovery_failed_event(
                &log_game_id,
                "auto_detect",
                &error.code,
            ));
            Err(error)
        }
    }
}

fn game_discovery_completed_event(
    game_id: &str,
    operation: &'static str,
    result: &str,
    candidate_count: usize,
) -> AppLogEvent {
    AppLogEvent::info("game.discovery.completed")
        .with_game_id(game_id)
        .with_operation(operation)
        .with_result(result)
        .with_item_count(u64::try_from(candidate_count).unwrap_or(u64::MAX))
}

fn game_discovery_failed_event(
    game_id: &str,
    operation: &'static str,
    error_code: &str,
) -> AppLogEvent {
    AppLogEvent::warning("game.discovery.failed")
        .with_game_id(game_id)
        .with_operation(operation)
        .with_result("failed")
        .with_error_code(error_code)
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

    #[test]
    fn discovery_summary_event_contains_only_stable_aggregates() {
        assert_eq!(
            game_discovery_completed_event("mhw", "scan_candidates", "success", 2),
            AppLogEvent::info("game.discovery.completed")
                .with_game_id("mhw")
                .with_operation("scan_candidates")
                .with_result("success")
                .with_item_count(2)
        );
        assert_eq!(
            game_discovery_failed_event("mhw", "auto_detect", "steam_discovery_unavailable"),
            AppLogEvent::warning("game.discovery.failed")
                .with_game_id("mhw")
                .with_operation("auto_detect")
                .with_result("failed")
                .with_error_code("steam_discovery_unavailable")
        );
    }
}
