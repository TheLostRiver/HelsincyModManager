import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "../mods/modImportTypes";
import type { ModStorageDirValidationDto, ModStorageSettingsDto } from "./modStorageTypes";

export function getModStorageSettings(): Promise<ModStorageSettingsDto> {
  return invoke<ModStorageSettingsDto>("get_mod_storage_settings");
}

/** 只读校验；`directory` 只能来自系统目录选择器，校验不通过以 `code` 表达、不抛错。 */
export function validateModStorageDir(directory: string): Promise<ModStorageDirValidationDto> {
  return invoke<ModStorageDirValidationDto>("validate_mod_storage_dir", { directory });
}

/** 库为空时直接持久化；`null` = 回到默认目录。库非空返回 `mod_storage_migration_required`。 */
export function setModStorageDir(directory: string | null): Promise<ModStorageSettingsDto> {
  return invoke<ModStorageSettingsDto>("set_mod_storage_dir", { directory });
}

/** 库非空时的换目录：登记 `mod_storage_migration` 任务，进度走 `hmm://task-progress`。 */
export function startModStorageMigrationTask(directory: string | null): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_mod_storage_migration_task", { directory });
}

export function cancelModStorageMigrationTask(taskId: string): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", { taskId });
}
