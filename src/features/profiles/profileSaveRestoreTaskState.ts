import type { TaskProgressEventDto, TaskStartedDto } from "../mods/modImportTypes";
import type { SaveRestoreCodeCopy, SaveRestoreCopy } from "./saveRestoreCopy";

export type ProfileSaveRestoreTaskPhase =
  | "save_restore.queued"
  | "save_restore.preparing"
  | "save_restore.revalidating"
  | "save_restore.pre_restore_backup"
  | "save_restore.committing"
  | "save_restore.completed"
  | "save_restore.failed"
  | "save_restore.recovery_required"
  | "save_restore.cancelled";

export type ProfileSaveRestoreRunningPhase = Exclude<
  ProfileSaveRestoreTaskPhase,
  | "save_restore.completed"
  | "save_restore.failed"
  | "save_restore.recovery_required"
  | "save_restore.cancelled"
>;

export type ProfileSaveRestoreTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: ProfileSaveRestoreRunningPhase }
  | { status: "cancelling"; taskId: string; phase: ProfileSaveRestoreRunningPhase }
  | {
      status: "completed";
      taskId: string;
      evidenceDegraded: boolean;
      warningCodes: string[];
    }
  | {
      status: "failed";
      taskId: string | null;
      errorCode: string | null;
      warningCodes: string[];
    }
  | { status: "recovery_required"; taskId: string; errorCode: string }
  | { status: "cancelled"; taskId: string };

const PROFILE_SAVE_RESTORE_EVIDENCE_DEGRADED = "save_restore_evidence_degraded";
const DEFAULT_EARLY_EVENT_TASK_LIMIT = 8;
const DEFAULT_EARLY_EVENT_PER_TASK_LIMIT = 12;

// 语义映射：phase -> 事件 status 的一致性判定，与文案无关。
const phaseStatuses: Record<ProfileSaveRestoreTaskPhase, TaskProgressEventDto["status"]> = {
  "save_restore.queued": "queued",
  "save_restore.preparing": "running",
  "save_restore.revalidating": "running",
  "save_restore.pre_restore_backup": "running",
  "save_restore.committing": "running",
  "save_restore.completed": "completed",
  "save_restore.failed": "failed",
  "save_restore.recovery_required": "failed",
  "save_restore.cancelled": "cancelled",
};

export function isProfileSaveRestoreTaskPhase(phase: string): phase is ProfileSaveRestoreTaskPhase {
  return Object.prototype.hasOwnProperty.call(phaseStatuses, phase);
}

export function isProfileSaveRestoreProgressEvent(event: TaskProgressEventDto) {
  return event.kind === "save_restore"
    && isProfileSaveRestoreTaskPhase(event.phase)
    && phaseStatuses[event.phase] === event.status;
}

export function isProfileSaveRestoreTaskStarted(task: TaskStartedDto) {
  return task.kind === "save_restore" && task.status === "queued";
}

export function createProfileSaveRestoreRunningState(taskId: string): ProfileSaveRestoreTaskState {
  return { status: "running", taskId, phase: "save_restore.queued" };
}

export function attachProfileSaveRestoreTask(
  taskId: string,
  earlyEvents: readonly TaskProgressEventDto[],
): ProfileSaveRestoreTaskState {
  return earlyEvents.reduce(
    (state, event) => nextProfileSaveRestoreTaskStateFromProgress(state, event),
    createProfileSaveRestoreRunningState(taskId),
  );
}

export function nextProfileSaveRestoreTaskStateFromProgress(
  current: ProfileSaveRestoreTaskState,
  event: TaskProgressEventDto,
): ProfileSaveRestoreTaskState {
  if (!isProfileSaveRestoreProgressEvent(event) || !("taskId" in current) || current.taskId !== event.taskId) {
    return current;
  }
  if (isProfileSaveRestoreTerminalState(current)
    && !(current.status === "cancelled" && event.phase === "save_restore.recovery_required")) {
    return current;
  }

  if (event.phase === "save_restore.completed") {
    const warningCodes = [stableCode(event.error), stableCode(event.message)].filter(
      (value): value is string => value !== null,
    );
    return {
      status: "completed",
      taskId: event.taskId,
      evidenceDegraded: warningCodes.includes(PROFILE_SAVE_RESTORE_EVIDENCE_DEGRADED),
      warningCodes: [...new Set(warningCodes)],
    };
  }

  if (event.phase === "save_restore.recovery_required") {
    const errorCode = stableCode(event.error) ?? "save_restore_recovery_required";
    return {
      status: "recovery_required",
      taskId: event.taskId,
      errorCode,
    };
  }

  if (event.phase === "save_restore.failed") {
    const errorCode = stableCode(event.error) ?? stableCode(event.message);
    const warningCode = stableCode(event.error) ? stableCode(event.message) : null;
    return {
      status: "failed",
      taskId: event.taskId,
      errorCode,
      warningCodes: warningCode && warningCode !== errorCode ? [warningCode] : [],
    };
  }

  if (event.phase === "save_restore.cancelled") {
    return { status: "cancelled", taskId: event.taskId };
  }

  return {
    status: current.status === "cancelling" ? "cancelling" : "running",
    taskId: event.taskId,
    phase: event.phase as ProfileSaveRestoreRunningPhase,
  };
}

export function markProfileSaveRestoreCancelling(
  state: ProfileSaveRestoreTaskState,
): ProfileSaveRestoreTaskState {
  if (state.status !== "running" || !canCancelProfileSaveRestore(state)) return state;
  return { ...state, status: "cancelling" };
}

export function canCancelProfileSaveRestore(state: ProfileSaveRestoreTaskState) {
  return state.status === "running" && state.phase !== "save_restore.committing";
}

export function getProfileSaveRestorePhaseLabel(
  phase: ProfileSaveRestoreRunningPhase,
  phaseLabels: SaveRestoreCopy["phases"],
) {
  return phaseLabels[phase];
}

export function getProfileSaveRestoreErrorCode(error: unknown) {
  if (typeof error === "string") return stableCode(error);
  if (error && typeof error === "object" && "code" in error) {
    return stableCode(String(error.code));
  }
  return null;
}

export function getProfileSaveRestoreErrorMessage(error: unknown, errorCopy: SaveRestoreCodeCopy) {
  const code = getProfileSaveRestoreErrorCode(error);
  return code ? errorCopy.byCode[code] ?? errorCopy.fallback : errorCopy.fallback;
}

export function getProfileSaveRestoreWarningMessage(code: string, warningCopy: SaveRestoreCodeCopy) {
  return warningCopy.byCode[code] ?? warningCopy.fallback;
}

export function isProfileSaveRestoreTerminalState(state: ProfileSaveRestoreTaskState) {
  return state.status === "completed"
    || state.status === "failed"
    || state.status === "recovery_required"
    || state.status === "cancelled";
}

export class ProfileSaveRestoreEarlyEventBuffer {
  #eventsByTask = new Map<string, TaskProgressEventDto[]>();
  #maxTasks: number;
  #maxEventsPerTask: number;

  constructor(
    maxTasks = DEFAULT_EARLY_EVENT_TASK_LIMIT,
    maxEventsPerTask = DEFAULT_EARLY_EVENT_PER_TASK_LIMIT,
  ) {
    this.#maxTasks = Math.max(1, maxTasks);
    this.#maxEventsPerTask = Math.max(1, maxEventsPerTask);
  }

  push(event: TaskProgressEventDto) {
    if (!isProfileSaveRestoreProgressEvent(event)) return;
    if (!this.#eventsByTask.has(event.taskId) && this.#eventsByTask.size >= this.#maxTasks) {
      const oldestTaskId = this.#eventsByTask.keys().next().value;
      if (oldestTaskId) this.#eventsByTask.delete(oldestTaskId);
    }
    const events = this.#eventsByTask.get(event.taskId) ?? [];
    events.push(event);
    if (events.length > this.#maxEventsPerTask) events.splice(0, events.length - this.#maxEventsPerTask);
    this.#eventsByTask.set(event.taskId, events);
  }

  take(taskId: string) {
    const events = this.#eventsByTask.get(taskId) ?? [];
    this.#eventsByTask.delete(taskId);
    return [...events];
  }

  clear() {
    this.#eventsByTask.clear();
  }
}

function stableCode(value: string | null) {
  if (!value || value.length > 96) return null;
  return /^[A-Za-z0-9_.:-]+$/.test(value) ? value : null;
}
