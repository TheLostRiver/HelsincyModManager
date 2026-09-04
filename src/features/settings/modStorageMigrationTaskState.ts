// 直连 .ts：本模块被 node --test 直接 import。
import type { TaskProgressEventDto } from "../mods/modImportTypes";
import type { ModStorageCopy, ModStorageMigrationPhase } from "./modStorageCopy.ts";

/**
 * 迁移任务在前端的投影（契约「Mod 存储目录迁移（#275 切片②）」phase 表）。
 * 事件只带任务身份、phase、`current / total`（包计数）与稳定码；这里不解释任何路径。
 */
export type ModStorageMigrationTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | {
      status: "running";
      taskId: string;
      phase: string;
      current: number | null;
      total: number | null;
    }
  | {
      status: "cancelling";
      taskId: string;
      phase: string;
      current: number | null;
      total: number | null;
    }
  | { status: "completed"; taskId: string; packageCount: number | null }
  | { status: "cancelled"; taskId: string }
  | { status: "failed"; taskId: string | null; errorCode: string };

export const MOD_STORAGE_MIGRATION_PHASES = {
  queued: "mod_storage.migration.queued",
  copying: "mod_storage.migration.copying",
  verifying: "mod_storage.migration.verifying",
  switching: "mod_storage.migration.switching",
  completed: "mod_storage.migration.completed",
  failed: "mod_storage.migration.failed",
  cancelling: "mod_storage.migration.cancelling",
  cancelled: "mod_storage.migration.cancelled",
} as const satisfies Record<string, ModStorageMigrationPhase>;

const knownPhases: ReadonlySet<string> = new Set(Object.values(MOD_STORAGE_MIGRATION_PHASES));

/** failed 事件允许携带的稳定码；其余一律折叠为「进度不可识别」，不把未知字串当码用。 */
const migrationFailureCodes: ReadonlySet<string> = new Set([
  "mod_storage_migration_source_unavailable",
  "mod_storage_migration_target_unavailable",
  "mod_storage_migration_package_unreadable",
  "mod_storage_migration_copy_failed",
  "mod_storage_migration_verify_mismatch",
  "mod_storage_migration_journal_unavailable",
  "mod_storage_migration_settings_unavailable",
]);

export const MOD_STORAGE_MIGRATION_UNRECOGNIZED_CODE = "mod_storage_migration_progress_unrecognized";

export function isModStorageMigrationTerminal(state: ModStorageMigrationTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

export function isModStorageMigrationActive(state: ModStorageMigrationTaskState) {
  return state.status === "starting" || state.status === "running" || state.status === "cancelling";
}

/** 切换设置阶段已进入取消屏障，后端会拒绝取消；UI 据此禁用按钮而不是等报错。 */
export function canCancelModStorageMigration(state: ModStorageMigrationTaskState) {
  return state.status === "running" && state.phase !== MOD_STORAGE_MIGRATION_PHASES.switching;
}

export function getModStorageMigrationPhaseLabel(
  phase: string,
  migrationCopy: ModStorageCopy["migration"],
) {
  return knownPhases.has(phase)
    ? migrationCopy.phases[phase as ModStorageMigrationPhase]
    : migrationCopy.unrecognizedPhase;
}

function isSafeCount(value: number) {
  return Number.isSafeInteger(value) && value >= 0;
}

function hasSafeProgress(event: TaskProgressEventDto) {
  if (event.current === null || event.total === null) {
    return event.current === null && event.total === null;
  }
  return isSafeCount(event.current) && isSafeCount(event.total) && event.current <= event.total;
}

function failedState(taskId: string, errorCode = MOD_STORAGE_MIGRATION_UNRECOGNIZED_CODE): ModStorageMigrationTaskState {
  return { status: "failed", taskId, errorCode };
}

export function nextModStorageMigrationStateFromProgress(
  current: ModStorageMigrationTaskState,
  event: TaskProgressEventDto,
): ModStorageMigrationTaskState {
  if (isModStorageMigrationTerminal(current)) {
    return current;
  }
  if (
    event.kind !== "mod_storage_migration" ||
    !("taskId" in current) ||
    current.taskId !== event.taskId
  ) {
    return current;
  }
  if (!knownPhases.has(event.phase) || !hasSafeProgress(event)) {
    return failedState(event.taskId);
  }

  switch (event.phase) {
    case MOD_STORAGE_MIGRATION_PHASES.queued:
      return event.status === "queued" && event.current === null
        ? { status: "running", taskId: event.taskId, phase: event.phase, current: null, total: null }
        : failedState(event.taskId);
    case MOD_STORAGE_MIGRATION_PHASES.copying:
    case MOD_STORAGE_MIGRATION_PHASES.verifying:
    case MOD_STORAGE_MIGRATION_PHASES.switching:
      return event.status === "running" && event.current !== null && event.total !== null
        ? {
            status: "running",
            taskId: event.taskId,
            phase: event.phase,
            current: event.current,
            total: event.total,
          }
        : failedState(event.taskId);
    case MOD_STORAGE_MIGRATION_PHASES.cancelling:
      // cancel_task 受理后的即时事件：副本仍在删除，写门闩仍冻结，终态要等 cancelled。
      return event.status === "cancelled"
        ? {
            status: "cancelling",
            taskId: event.taskId,
            phase: event.phase,
            current: current.status === "running" ? current.current : null,
            total: current.status === "running" ? current.total : null,
          }
        : failedState(event.taskId);
    case MOD_STORAGE_MIGRATION_PHASES.completed:
      return event.status === "completed"
        ? { status: "completed", taskId: event.taskId, packageCount: event.current }
        : failedState(event.taskId);
    case MOD_STORAGE_MIGRATION_PHASES.cancelled:
      return event.status === "cancelled"
        ? { status: "cancelled", taskId: event.taskId }
        : failedState(event.taskId);
    case MOD_STORAGE_MIGRATION_PHASES.failed:
      return event.status === "failed"
        ? failedState(
            event.taskId,
            event.error !== null && migrationFailureCodes.has(event.error)
              ? event.error
              : MOD_STORAGE_MIGRATION_UNRECOGNIZED_CODE,
          )
        : failedState(event.taskId);
    default:
      return failedState(event.taskId);
  }
}
