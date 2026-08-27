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

/**
 * 在系统文件管理器中打开该 profile 已配置的存档或备份目录。
 *
 * 刻意只传 `kind` 而不传路径:真实路径由后端从持久化事实解析并校验(拒 symlink /
 * 重解析点 / 非目录),前端全程不经手路径,这个入口也就无法用来打开任意位置。
 */
export function openProfileDirectory(input: {
  gameId: string;
  profileId: string;
  kind: "save" | "backup";
}): Promise<void> {
  return invoke<void>("open_profile_directory", input);
}
