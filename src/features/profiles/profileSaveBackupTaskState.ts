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
  | {
      status: "failed";
      taskId: string | null;
      phase: "save_backup.failed";
      errorCode: string | null;
      message: string;
    }
  | { status: "cancelled"; taskId: string; phase: "save_backup.cancelled" };

const errorMessages: Record<string, string> = {
  write_admission_busy: "另一项存档操作正在进行，请稍后再试。",
  write_admission_cancelled: "存档备份已取消。",
  write_admission_order_violation: "存档操作顺序发生变化，请稍后重试。",
  write_admission_unavailable: "暂时无法锁定存档写入，请稍后重试。",
  save_backup_profile_missing: "当前配置档已不存在，请刷新后重试。",
  save_backup_source_unset: "当前配置档尚未设置存档目录。",
  save_backup_source_invalid: "当前配置档的存档目录无效，请先重新设置。",
  save_backup_clock_unavailable: "无法建立可靠的备份时间，请稍后重试。",
  save_backup_destination_unavailable: "备份目录当前不可用，请检查目录设置。",
  save_backup_archive_write_failed: "无法写入存档备份，请检查备份目录。",
  save_backup_history_unavailable: "备份历史当前不可用，请稍后重试。",
  save_backup_retention_failed: "备份保留策略执行失败，请检查备份中心。",
  save_backup_scheduler_lease_unavailable: "自动备份调度状态暂时不可用，请稍后重试。",
};

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
  return "存档备份失败，请稍后重试。";
}

export function getProfileSaveBackupTaskErrorCode(error: unknown) {
  if (typeof error === "string") return normalizeProfileSaveBackupErrorCode(error);
  if (error && typeof error === "object" && "code" in error) {
    return normalizeProfileSaveBackupErrorCode(String(error.code));
  }
  return null;
}

export function getProfileSaveBackupTaskErrorMessage(error: unknown) {
  const code = getProfileSaveBackupTaskErrorCode(error);
  return code ? errorMessages[code] ?? defaultProfileSaveBackupTaskErrorMessage() : defaultProfileSaveBackupTaskErrorMessage();
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
    const errorCode = getProfileSaveBackupTaskErrorCode(event.error)
      ?? getProfileSaveBackupTaskErrorCode(event.message);
    return {
      status: "failed",
      taskId: event.taskId,
      phase,
      errorCode,
      message: getProfileSaveBackupTaskErrorMessage(errorCode),
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

function normalizeProfileSaveBackupErrorCode(value: string) {
  const trimmed = value.trim();
  const prefix = "save_backup_failed:";
  const candidate = trimmed.startsWith(prefix) ? trimmed.slice(prefix.length) : trimmed;
  if (!candidate || candidate.length > 96) return null;
  return /^[a-z][a-z0-9_]*$/.test(candidate) ? candidate : null;
}
