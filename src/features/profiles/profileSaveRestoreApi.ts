import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "../mods/modImportTypes";
import type { SaveRestorePreviewDto, SaveRestoreTaskStartedDto } from "./profileSaveRestoreTypes";

export function previewProfileSaveRestore(input: {
  gameId: string;
  profileId: string;
  backupId: string;
}): Promise<SaveRestorePreviewDto> {
  return invoke<SaveRestorePreviewDto>("preview_save_restore", { request: input });
}

export function startProfileSaveRestore(input: {
  gameId: string;
  profileId: string;
  backupId: string;
  previewToken: string;
  confirmedWithoutPreRestore: boolean;
}): Promise<SaveRestoreTaskStartedDto> {
  return invoke<SaveRestoreTaskStartedDto>("start_save_restore_task", {
    request: {
      ...input,
      confirmed: true,
    },
  });
}

export function cancelProfileSaveRestoreTask(taskId: string): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", { taskId });
}
