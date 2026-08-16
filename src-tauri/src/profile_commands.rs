use crate::dto::{
    CommandErrorDto, ProfileDirectorySelectionDto, ProfileDto, ProfileSaveSettingsDto,
    SetProfileSaveSettingsRequestDto,
};
use crate::state::AppState;
use hmm_app::{CreateProfileRequest, SetProfileSaveSettingsRequest, UpdateProfileRequest};
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

#[tauri::command]
pub fn get_profile_save_settings(
    game_id: String,
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<ProfileSaveSettingsDto, CommandErrorDto> {
    state
        .profiles
        .get_profile_save_settings(&game_id, &profile_id)
        .map(ProfileSaveSettingsDto::from)
        .map_err(profile_error)
}

#[tauri::command]
pub fn validate_profile_save_directory(
    game_id: String,
    profile_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<ProfileDirectorySelectionDto, CommandErrorDto> {
    state
        .profiles
        .get_profile_save_settings(&game_id, &profile_id)
        .and_then(|_| {
            state
                .profiles
                .validate_profile_save_directory(&game_id, &directory)
        })
        .map(ProfileDirectorySelectionDto::from)
        .map_err(profile_error)
}

#[tauri::command]
pub fn validate_profile_backup_directory(
    game_id: String,
    profile_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<ProfileDirectorySelectionDto, CommandErrorDto> {
    state
        .profiles
        .get_profile_save_settings(&game_id, &profile_id)
        .and_then(|_| {
            state
                .profiles
                .validate_profile_backup_directory(&game_id, &directory)
        })
        .map(ProfileDirectorySelectionDto::from)
        .map_err(profile_error)
}

#[tauri::command]
pub fn set_profile_save_settings(
    input: SetProfileSaveSettingsRequestDto,
    state: State<'_, AppState>,
) -> Result<ProfileSaveSettingsDto, CommandErrorDto> {
    state
        .profiles
        .set_profile_save_settings(SetProfileSaveSettingsRequest {
            profile_id: input.profile_id,
            game_id: input.game_id,
            save_directory: input.save_directory,
            backup_directory: input.backup_directory,
            schedule: input.schedule.into(),
            retention: input.retention.into(),
            pre_restore_backup_enabled: input.pre_restore_backup_enabled,
        })
        .map(ProfileSaveSettingsDto::from)
        .map_err(profile_error)
}
