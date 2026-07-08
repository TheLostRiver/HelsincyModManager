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

export type GetSaveBackupBackgroundStatusInput = {
  gameId: string;
  profileId: string;
};

export type SaveBackupBackgroundStatus =
  | "protected"
  | "tray_only"
  | "not_enabled"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform";

export type SaveBackupPendingReason =
  | "game_running"
  | "game_running_unknown"
  | "source_invalid"
  | "destination_unavailable"
  | "task_conflict";

export type SaveBackupBackgroundStatusDto = {
  gameId: string;
  profileId: string;
  status: SaveBackupBackgroundStatus;
  backgroundProtectionEnabled: boolean;
  lastCheckedAt: number | null;
  lastAttemptAt: number | null;
  lastSuccessAt: number | null;
  nextDueAt: number | null;
  pendingReason: SaveBackupPendingReason | null;
  lastErrorCode: string | null;
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
