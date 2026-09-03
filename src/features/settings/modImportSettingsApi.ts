import { invoke } from "@tauri-apps/api/core";
import type { ModImportSettingsDto } from "./modImportSettingsTypes";

export function getModImportSettings(): Promise<ModImportSettingsDto> {
  return invoke<ModImportSettingsDto>("get_mod_import_settings");
}

/** Only the flag travels; the deletion itself is decided and performed by the import runner. */
export function setModImportSettings(deleteArchiveAfterImport: boolean): Promise<ModImportSettingsDto> {
  return invoke<ModImportSettingsDto>("set_mod_import_settings", { deleteArchiveAfterImport });
}
