use crate::dto::{CommandErrorDto, ProfileDto};
use crate::state::AppState;
use hmm_app::{CreateProfileRequest, UpdateProfileRequest};
use tauri::State;

fn profile_error(error: impl std::fmt::Display) -> CommandErrorDto {
    CommandErrorDto {
        code: "profile_error".to_owned(),
        message: error.to_string(),
    }
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileDto>, CommandErrorDto> {
    state
        .profiles
        .list_profiles()
        .map(|profiles| profiles.into_iter().map(ProfileDto::from).collect())
        .map_err(profile_error)
}

#[tauri::command]
pub fn get_active_profile(state: State<'_, AppState>) -> Result<ProfileDto, CommandErrorDto> {
    state
        .profiles
        .get_active_profile()
        .map(ProfileDto::from)
        .map_err(profile_error)
}

#[tauri::command]
pub fn create_profile(
    name: String,
    description: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, CommandErrorDto> {
    state
        .profiles
        .create_profile(CreateProfileRequest { name, description })
        .map_err(profile_error)
}

#[tauri::command]
pub fn update_profile(
    profile_id: String,
    name: Option<String>,
    description: Option<Option<String>>,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    state
        .profiles
        .update_profile(UpdateProfileRequest {
            profile_id,
            name,
            description,
        })
        .map_err(profile_error)
}

#[tauri::command]
pub fn delete_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    state
        .profiles
        .delete_profile(&profile_id)
        .map_err(profile_error)
}

#[tauri::command]
pub fn set_active_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    state
        .profiles
        .set_active_profile(&profile_id)
        .map_err(profile_error)
}
