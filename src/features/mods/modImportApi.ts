import { invoke } from "@tauri-apps/api/core";
import type { DroppedArchivePreview } from "./modImportDropState";
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

/**
 * 拖拽进来的文件逐个预检（T22 / #366）。**只读，不启动任何任务。**
 *
 * 返回的 `errorCode` 与导入失败是同一套语义码，前端复用同一张档位映射表。
 */
export function previewDroppedModArchives(
  archivePaths: readonly string[],
): Promise<DroppedArchivePreview[]> {
  return invoke<DroppedArchivePreview[]>("preview_dropped_mod_archives", {
    archivePaths: [...archivePaths],
  });
}

export function cancelImportTask(input: CancelTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", {
    taskId: input.taskId,
  });
}
