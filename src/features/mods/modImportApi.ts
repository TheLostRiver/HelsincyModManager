import { invoke } from "@tauri-apps/api/core";
import type { StartImportModTaskInput, TaskStartedDto } from "./modImportTypes";

export function startImportModTask(input: StartImportModTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_import_mod_task", {
    archivePath: input.archivePath,
  });
}
