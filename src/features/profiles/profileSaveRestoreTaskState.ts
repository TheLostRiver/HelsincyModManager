import type { TaskProgressEventDto, TaskStartedDto } from "../mods/modImportTypes";

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

type ProfileSaveRestoreRunningPhase = Exclude<
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
      message: string;
      warningCodes: string[];
    }
  | { status: "recovery_required"; taskId: string; errorCode: string; message: string }
  | { status: "cancelled"; taskId: string };

const PROFILE_SAVE_RESTORE_EVIDENCE_DEGRADED = "save_restore_evidence_degraded";
const DEFAULT_EARLY_EVENT_TASK_LIMIT = 8;
const DEFAULT_EARLY_EVENT_PER_TASK_LIMIT = 12;

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

const phaseLabels: Record<ProfileSaveRestoreRunningPhase, string> = {
  "save_restore.queued": "等待恢复任务",
  "save_restore.preparing": "正在校验并准备存档",
  "save_restore.revalidating": "正在复核目标状态",
  "save_restore.pre_restore_backup": "正在创建恢复前安全备份",
  "save_restore.committing": "正在替换并校验存档",
};

const errorMessages: Record<string, string> = {
  save_restore_profile_missing: "配置档已不存在，请刷新后重试。",
  save_restore_backup_missing: "所选备份记录已不存在，请刷新备份历史。",
  save_restore_backup_unavailable: "所选备份当前不可用于恢复。",
  save_restore_target_unset: "当前配置档尚未设置存档目录。",
  save_restore_target_invalid: "当前配置档的存档目录无效，请先重新设置。",
  save_restore_game_running: "游戏仍在运行，请完全退出游戏后重试。",
  save_restore_game_running_unknown: "无法确认游戏是否已退出，恢复已安全阻断。",
  save_restore_source_invalid: "备份归档或清单未通过安全校验。",
  save_restore_backup_directory_unavailable: "备份目录当前不可读取。",
  save_restore_archive_unavailable: "备份归档文件当前不可读取。",
  save_restore_manifest_unavailable: "备份清单当前不可读取。",
  save_restore_manifest_invalid: "备份清单无效，不能用于恢复。",
  save_restore_archive_invalid: "备份归档无效，不能用于恢复。",
  save_restore_hash_mismatch: "备份内容校验不一致，恢复已停止。",
  save_restore_path_unsafe: "备份包含不安全路径，恢复已停止。",
  save_restore_size_limit_exceeded: "备份内容超过恢复安全限制。",
  save_restore_staging_unavailable: "无法创建受控恢复暂存区。",
  save_restore_recovery_required: "恢复未能安全收敛，已保留恢复证据。",
  save_restore_transaction_unavailable: "无法持久化恢复事务，恢复已安全停止。",
  save_restore_clock_unavailable: "无法建立可靠的恢复时间事实。",
  save_restore_token_issue_failed: "无法创建恢复预览凭证，请重新打开面板。",
  save_restore_token_invalid: "恢复预览凭证无效，请重新打开面板。",
  save_restore_token_expired: "恢复预览已过期，请重新打开面板。",
  save_restore_token_stale: "恢复预览后的事实已变化，请重新打开面板。",
  save_restore_confirmation_required: "恢复需要明确确认。",
  save_restore_high_risk_confirmation_required: "关闭恢复前安全备份时需要额外确认。",
  save_restore_pre_restore_backup_invalid: "恢复前安全备份未通过校验，未写入当前存档。",
  save_restore_facts_changed: "存档或备份事实已变化，请重新预览。",
  save_restore_lock_unavailable: "当前配置档正在执行其他写入操作。",
  save_restore_prepared_missing: "恢复暂存内容已失效，请重新预览。",
  save_restore_target_unavailable: "目标存档目录当前不可用。",
  save_restore_target_unsafe: "目标存档目录未通过安全校验。",
  save_restore_target_changed: "目标存档在预览后发生变化，请重新预览。",
  save_restore_commit_failed: "恢复提交失败，当前存档未被视为成功恢复。",
  save_restore_rolled_back: "恢复未完成，已自动恢复到操作前存档。",
  save_backup_history_unavailable: "恢复前安全备份失败，未写入当前存档。",
};

const warningMessages: Record<string, string> = {
  [PROFILE_SAVE_RESTORE_EVIDENCE_DEGRADED]: "任务或审计证据记录不完整，请保留诊断信息。",
  save_restore_recovery_cleanup_failed: "恢复证据未能自动清理，请保留现场并联系支持。",
  save_restore_recovery_evidence_unsafe: "恢复证据需要人工检查，请保留现场并联系支持。",
  save_restore_target_unavailable: "收尾时目标目录暂时不可用，请保留现场并联系支持。",
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
      message: getProfileSaveRestoreErrorMessage(errorCode),
    };
  }

  if (event.phase === "save_restore.failed") {
    const errorCode = stableCode(event.error) ?? stableCode(event.message);
    const warningCode = stableCode(event.error) ? stableCode(event.message) : null;
    return {
      status: "failed",
      taskId: event.taskId,
      errorCode,
      message: getProfileSaveRestoreErrorMessage(errorCode),
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

export function getProfileSaveRestorePhaseLabel(phase: ProfileSaveRestoreRunningPhase) {
  return phaseLabels[phase];
}

export function getProfileSaveRestoreErrorCode(error: unknown) {
  if (typeof error === "string") return stableCode(error);
  if (error && typeof error === "object" && "code" in error) {
    return stableCode(String(error.code));
  }
  return null;
}

export function getProfileSaveRestoreErrorMessage(error: unknown) {
  const code = getProfileSaveRestoreErrorCode(error);
  return code ? errorMessages[code] ?? "存档恢复失败，当前存档未被视为成功恢复。" : "存档恢复失败，当前存档未被视为成功恢复。";
}

export function getProfileSaveRestoreWarningMessage(code: string) {
  return warningMessages[code] ?? "恢复收尾证据需要检查，请保留现场并联系支持。";
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
