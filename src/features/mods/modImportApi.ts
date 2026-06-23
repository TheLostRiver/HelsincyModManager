import { invoke } from "@tauri-apps/api/core";
import type { CancelTaskInput, StartImportModTaskInput, TaskStartedDto } from "./modImportTypes";

export function startImportModTask(input: StartImportModTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_import_mod_task", {
    archivePath: input.archivePath,
  });
}

export function cancelImportTask(input: CancelTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", {
    taskId: input.taskId,
  });
}
