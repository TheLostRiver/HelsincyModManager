import type { TaskProgressEventDto } from "../mods/modImportTypes";

export type ProfileSaveBackupTaskPhase =
  | "save_backup.queued"
  | "save_backup.scanning"
  | "save_backup.archiving"
  | "save_backup.manifest_writing"
  | "save_backup.retention_pruning"
  | "save_backup.completed"
  | "save_backup.failed"
  | "save_backup.cancelled";

export type ProfileSaveBackupTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: Exclude<ProfileSaveBackupTaskPhase, "save_backup.completed" | "save_backup.failed" | "save_backup.cancelled"> }
  | { status: "completed"; taskId: string; phase: "save_backup.completed"; resultRef: string | null }
  | { status: "failed"; taskId: string | null; phase: "save_backup.failed"; message: string }
  | { status: "cancelled"; taskId: string; phase: "save_backup.cancelled" };

const profileSaveBackupTaskPhaseLabels: Record<ProfileSaveBackupTaskPhase, string> = {
  "save_backup.queued": "等待备份",
  "save_backup.scanning": "校验存档",
  "save_backup.archiving": "写入归档",
  "save_backup.manifest_writing": "写入备份清单",
  "save_backup.retention_pruning": "清理旧备份",
  "save_backup.completed": "备份完成",
  "save_backup.failed": "备份失败",
  "save_backup.cancelled": "已取消",
};

export function isProfileSaveBackupTaskPhase(phase: string): phase is ProfileSaveBackupTaskPhase {
  return Object.prototype.hasOwnProperty.call(profileSaveBackupTaskPhaseLabels, phase);
}

export function getProfileSaveBackupTaskPhaseLabel(phase: ProfileSaveBackupTaskPhase) {
  return profileSaveBackupTaskPhaseLabels[phase];
}

export function defaultProfileSaveBackupTaskErrorMessage() {
  return "存档备份失败";
}

export function nextProfileSaveBackupTaskStateFromProgress(
  current: ProfileSaveBackupTaskState,
  event: TaskProgressEventDto,
): ProfileSaveBackupTaskState {
  if (event.kind !== "save_backup" || !("taskId" in current) || current.taskId !== event.taskId) {
    return current;
  }

  const phase = event.phase;
  if (!isProfileSaveBackupTaskPhase(phase)) {
    return current;
  }

  if (phase === "save_backup.completed") {
    return {
      status: "completed",
      taskId: event.taskId,
      phase,
      resultRef: event.resultRef,
    };
  }

  if (phase === "save_backup.failed") {
    return {
      status: "failed",
      taskId: event.taskId,
      phase,
      message: event.error ?? event.message ?? defaultProfileSaveBackupTaskErrorMessage(),
    };
  }

  if (phase === "save_backup.cancelled") {
    return {
      status: "cancelled",
      taskId: event.taskId,
      phase,
    };
  }

  return {
    status: "running",
    taskId: event.taskId,
    phase,
  };
}

export function shouldRefreshProfileSaveBackupHistory(state: ProfileSaveBackupTaskState) {
  return state.status === "completed";
}
