use crate::dto::CommandErrorDto;
use crate::save_directory_discovery_dto::SaveDirectoryDiscoveryDto;
use crate::state::AppState;
use hmm_app::{
    ConfirmProfileSaveDirectoryCandidateRequest, DiscoverProfileSaveDirectoriesRequest,
    SaveDirectoryDiscoveryError,
};
use hmm_core::{GameId, ProfileId};
use tauri::State;

const SAVE_DIRECTORY_DISCOVERY_FAILED_MESSAGE: &str = "save directory discovery failed";

#[tauri::command]
pub fn discover_profile_save_directories(
    game_id: String,
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<SaveDirectoryDiscoveryDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let profile_id = parse_profile_id(profile_id)?;

    state
        .save_directory_discovery
        .discover(DiscoverProfileSaveDirectoriesRequest {
            game_id,
            profile_id,
        })
        .map(SaveDirectoryDiscoveryDto::from)
        .map_err(save_directory_discovery_error)
}

#[tauri::command]
pub fn confirm_profile_save_directory_candidate(
    discovery_id: String,
    candidate_id: String,
    state: State<'_, AppState>,
) -> Result<SaveDirectoryDiscoveryDto, CommandErrorDto> {
    let discovery_id = parse_required_string(
        discovery_id,
        "save_directory_discovery_discovery_id_invalid",
    )?;
    let candidate_id = parse_required_string(
        candidate_id,
        "save_directory_discovery_candidate_id_invalid",
    )?;

    state
        .save_directory_discovery
        .confirm_candidate(ConfirmProfileSaveDirectoryCandidateRequest {
            discovery_id,
            candidate_id,
        })
        .map(SaveDirectoryDiscoveryDto::from)
        .map_err(save_directory_discovery_error)
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|_| CommandErrorDto {
        code: "save_directory_discovery_game_id_invalid".to_owned(),
        message: SAVE_DIRECTORY_DISCOVERY_FAILED_MESSAGE.to_owned(),
    })
}

fn parse_profile_id(value: String) -> Result<ProfileId, CommandErrorDto> {
    parse_required_string(value, "save_directory_discovery_profile_id_invalid").map(ProfileId::new)
}

fn parse_required_string(value: String, code: &str) -> Result<String, CommandErrorDto> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(CommandErrorDto {
            code: code.to_owned(),
            message: SAVE_DIRECTORY_DISCOVERY_FAILED_MESSAGE.to_owned(),
        });
    }

    Ok(value)
}

fn save_directory_discovery_error(error: SaveDirectoryDiscoveryError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: SAVE_DIRECTORY_DISCOVERY_FAILED_MESSAGE.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_profile_id_trims_and_rejects_empty_values() {
        assert_eq!(
            parse_profile_id("  default  ".to_owned())
                .expect("profile id")
                .as_str(),
            "default"
        );

        let error = parse_profile_id("  ".to_owned()).expect_err("empty profile id");
        assert_eq!(error.code, "save_directory_discovery_profile_id_invalid");
        assert!(!error.message.contains('/') && !error.message.contains('\\'));
    }

    #[test]
    fn service_errors_map_to_generic_command_error_without_paths() {
        let error = save_directory_discovery_error(SaveDirectoryDiscoveryError::CandidateInvalid);

        assert_eq!(error.code, "save_directory_discovery_candidate_invalid");
        assert_eq!(error.message, SAVE_DIRECTORY_DISCOVERY_FAILED_MESSAGE);
        assert!(!error.message.contains('/') && !error.message.contains('\\'));
    }
}
