import { invoke } from "@tauri-apps/api/core";
import type {
  CancelTaskInput,
  StartImportModRevisionTaskInput,
  StartImportModTaskInput,
  TaskStartedDto,
} from "./modImportTypes";

export function startImportModTask(input: StartImportModTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_import_mod_task", {
    archivePath: input.archivePath,
  });
}

export function startImportModRevisionTask(
  input: StartImportModRevisionTaskInput,
): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_import_mod_revision_task", {
    request: {
      archivePath: input.archivePath,
      modId: input.modId,
    },
  });
}

export function cancelImportTask(input: CancelTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", {
    taskId: input.taskId,
  });
}
