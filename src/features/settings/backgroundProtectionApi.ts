import { invoke } from "@tauri-apps/api/core";
import type { BackgroundProtectionControlDto } from "./backgroundProtectionTypes";

export function getBackgroundProtectionControlStatus(): Promise<BackgroundProtectionControlDto> {
  return invoke<BackgroundProtectionControlDto>("get_save_backup_background_control_status");
}

export function enableBackgroundProtection(): Promise<BackgroundProtectionControlDto> {
  return invoke<BackgroundProtectionControlDto>("enable_save_backup_background_protection");
}

export function disableBackgroundProtection(): Promise<BackgroundProtectionControlDto> {
  return invoke<BackgroundProtectionControlDto>("disable_save_backup_background_protection");
}
