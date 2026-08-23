import type { TaskProgressEventDto } from "../mods/modImportTypes";
import type { SaveBackupCopy, SaveBackupErrorCopy } from "./saveBackupCopy";

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
    }
  | { status: "cancelled"; taskId: string; phase: "save_backup.cancelled" };

// 语义 Set：阶段判定不依赖任何文案表，文本在渲染时经 copy 取。
const profileSaveBackupTaskPhases: ReadonlySet<string> = new Set<ProfileSaveBackupTaskPhase>([
  "save_backup.queued",
  "save_backup.scanning",
  "save_backup.archiving",
  "save_backup.manifest_writing",
  "save_backup.retention_pruning",
  "save_backup.completed",
  "save_backup.failed",
  "save_backup.cancelled",
]);

export function isProfileSaveBackupTaskPhase(phase: string): phase is ProfileSaveBackupTaskPhase {
  return profileSaveBackupTaskPhases.has(phase);
}

export function getProfileSaveBackupTaskPhaseLabel(
  phase: ProfileSaveBackupTaskPhase,
  phaseLabels: SaveBackupCopy["phases"],
) {
  return phaseLabels[phase];
}

export function getProfileSaveBackupTaskErrorCode(error: unknown) {
  if (typeof error === "string") return normalizeProfileSaveBackupErrorCode(error);
  if (error && typeof error === "object" && "code" in error) {
    return normalizeProfileSaveBackupErrorCode(String(error.code));
  }
  return null;
}

export function getProfileSaveBackupTaskErrorMessage(error: unknown, errorCopy: SaveBackupErrorCopy) {
  const code = getProfileSaveBackupTaskErrorCode(error);
  return code ? errorCopy.byCode[code] ?? errorCopy.fallback : errorCopy.fallback;
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
