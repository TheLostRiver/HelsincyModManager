import { invoke } from "@tauri-apps/api/core";
import type { DebugLogSettingsDto } from "./debugLogSettingsTypes";

export function getDebugLogSettings(): Promise<DebugLogSettingsDto> {
  return invoke<DebugLogSettingsDto>("get_debug_log_settings");
}

export function setDebugLogSettings(enabled: boolean): Promise<DebugLogSettingsDto> {
  return invoke<DebugLogSettingsDto>("set_debug_log_settings", { enabled });
}
