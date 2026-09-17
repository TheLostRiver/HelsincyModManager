import { invoke } from "@tauri-apps/api/core";
import type { TaskProgressEventDto } from "./modImportTypes";

export function getTaskProgress(taskId: string): Promise<TaskProgressEventDto | null> {
  return invoke<TaskProgressEventDto | null>("get_task_progress", { taskId });
}
