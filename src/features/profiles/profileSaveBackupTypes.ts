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

export type TaskStartedDto = {
  taskId: string;
  kind: "save_backup";
  status: "queued";
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
