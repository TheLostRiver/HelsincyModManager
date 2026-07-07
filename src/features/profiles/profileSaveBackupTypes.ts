export type StartProfileSaveBackupInput = {
  gameId: string;
  profileId: string;
  note?: string | null;
};

export type ListProfileSaveBackupsInput = {
  gameId: string;
  profileId: string;
  limit?: number | null;
};

export type CheckProfileAutoSaveBackupInput = {
  gameId: string;
  profileId: string;
};

export type TaskStartedDto = {
  taskId: string;
  kind: "save_backup";
  status: "queued";
};

export type ProfileAutoSaveBackupCheckDto = {
  gameId: string;
  profileId: string;
  clientRuntimeOnly: true;
  status: "manual_only" | "not_due" | "due";
  checkedAt: number;
  lastDueAt: number | null;
  nextDueAt: number | null;
  lastAutoBackupAt: number | null;
  startedTask: TaskStartedDto | null;
};

export type SaveBackupSummaryDto = {
  backupId: string;
  gameId: string;
  profileId: string;
  trigger: "manual" | "auto" | "pre_install";
  status: "completed" | "deleted_by_retention" | "missing" | "invalid";
  fileName: string;
  createdAt: number;
  sizeBytes: number;
  fileCount: number;
  sourcePathLabel: string | null;
  notes: string | null;
};
