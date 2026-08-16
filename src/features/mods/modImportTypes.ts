export type StartImportModTaskInput = {
  archivePath: string;
};

export type StartImportModRevisionTaskInput = StartImportModTaskInput & {
  modId: string;
};

export type CancelTaskInput = {
  taskId: string;
};

export const TASK_PROGRESS_EVENT_NAME = "hmm://task-progress";

export type TaskKind = "mod_import" | "install" | "save_backup" | "save_restore";
export type TaskStatus = "queued" | "running" | "completed" | "failed" | "cancelled";

export type TaskStartedDto = {
  taskId: string;
  kind: TaskKind;
  status: TaskStatus;
};

export type TaskProgressEventDto = {
  taskId: string;
  kind: TaskKind;
  status: TaskStatus;
  phase: string;
  current: number | null;
  total: number | null;
  message: string | null;
  error: string | null;
  resultRef: string | null;
};
