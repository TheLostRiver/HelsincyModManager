import { invoke } from "@tauri-apps/api/core";
import type {
  ProfileDirectoryValidationDto,
  ProfileSaveSettingsDto,
  SetProfileSaveSettingsInput,
} from "./profileSaveSettingsTypes";

export function getProfileSaveSettings(input: {
  gameId: string;
  profileId: string;
}): Promise<ProfileSaveSettingsDto> {
  return invoke<ProfileSaveSettingsDto>("get_profile_save_settings", input);
}

export function validateProfileSaveDirectory(input: {
  gameId: string;
  profileId: string;
  directory: string;
}): Promise<ProfileDirectoryValidationDto> {
  return invoke<ProfileDirectoryValidationDto>("validate_profile_save_directory", input);
}

export function validateProfileBackupDirectory(input: {
  gameId: string;
  profileId: string;
  directory: string;
}): Promise<ProfileDirectoryValidationDto> {
  return invoke<ProfileDirectoryValidationDto>("validate_profile_backup_directory", input);
}

export function setProfileSaveSettings(
  input: SetProfileSaveSettingsInput,
): Promise<ProfileSaveSettingsDto> {
  return invoke<ProfileSaveSettingsDto>("set_profile_save_settings", { input });
}
