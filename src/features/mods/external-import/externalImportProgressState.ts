import {
  isSafeNonNegativeInteger,
} from "./externalImportTypes.ts";
import type { TaskProgressEventDto } from "../modImportTypes";

export type ExternalImportTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | {
      status: "running";
      taskId: string;
      phase: string;
      current: number | null;
      total: number | null;
    }
  | { status: "cancelling"; taskId: string; phase: string }
  | { status: "completed"; taskId: string; phase: string }
  | { status: "cancelled"; taskId: string; phase: string }
  | { status: "failed"; taskId: string | null; phase: string; errorCode: string };

import type { ExternalImportCopy } from "./externalImportCopy";

const importPhases: ReadonlySet<string> = new Set([
  "external_import.import.queued",
  "external_import.import.materializing",
  "external_import.import.preparing",
  "external_import.import.persisting",
  "external_import.import.completed",
  "external_import.import.failed",
  "external_import.import.cancelled",
  "mod_import.cancelled",
]);

const stableImportErrorCodes = new Set([
  "external_import_source_unavailable",
  "external_import_task_unavailable",
  "external_import_batch_unavailable",
  "external_import_selection_unavailable",
  "external_import_batch_not_startable",
  "external_import_catalog_unavailable",
  "external_import_category_unavailable",
  "external_import_clock_unavailable",
  "selection_revision_conflict",
  "selection_empty",
  "selection_total_limit_exceeded",
  "selection_resource_limit_exceeded",
  "selection_candidate_invalid",
  "selection_expired",
  "selection_closed",
]);

export function getExternalImportPhaseLabel(phase: string, progress: ExternalImportCopy["progress"]) {
  return progress.phases[phase] ?? progress.unrecognized;
}

export function isExternalImportTaskTerminal(state: ExternalImportTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

function failedState(
  taskId: string,
  phase: string,
  errorCode = "external_import_progress_unrecognized",
): ExternalImportTaskState {
  return { status: "failed", taskId, phase, errorCode };
}

function hasSafeAggregateProgress(event: TaskProgressEventDto) {
  if (event.current === null || event.total === null) {
    return event.current === null && event.total === null;
  }
  return (
    isSafeNonNegativeInteger(event.current) &&
    isSafeNonNegativeInteger(event.total) &&
    event.current <= event.total
  );
}

function safeImportErrorCode(value: string | null) {
  return value !== null && stableImportErrorCodes.has(value)
    ? value
    : "external_import_batch_failed";
}

export function nextExternalImportTaskStateFromProgress(
  current: ExternalImportTaskState,
  event: TaskProgressEventDto,
): ExternalImportTaskState {
  if (isExternalImportTaskTerminal(current)) {
    return current;
  }
  if (
    event.kind !== "mod_import" ||
    !("taskId" in current) ||
    current.taskId !== event.taskId
  ) {
    return current;
  }
  if (!importPhases.has(event.phase) || !hasSafeAggregateProgress(event)) {
    return failedState(event.taskId, "external_import.import.unrecognized");
  }

  if (event.phase === "mod_import.cancelled") {
    return event.status === "cancelled"
      ? { status: "cancelling", taskId: event.taskId, phase: event.phase }
      : failedState(event.taskId, event.phase);
  }

  if (event.phase === "external_import.import.queued") {
    return event.status === "queued" && event.current === null && event.total === null
      ? {
          status: "running",
          taskId: event.taskId,
          phase: event.phase,
          current: null,
          total: null,
        }
      : failedState(event.taskId, event.phase);
  }

  if (
    event.phase === "external_import.import.materializing" ||
    event.phase === "external_import.import.preparing" ||
    event.phase === "external_import.import.persisting"
  ) {
    return event.status === "running" && event.current !== null && event.total !== null
      ? {
          status: "running",
          taskId: event.taskId,
          phase: event.phase,
          current: event.current,
          total: event.total,
        }
      : failedState(event.taskId, event.phase);
  }

  if (event.phase === "external_import.import.completed") {
    return event.status === "completed"
      ? { status: "completed", taskId: event.taskId, phase: event.phase }
      : failedState(event.taskId, event.phase);
  }
  if (event.phase === "external_import.import.cancelled") {
    return event.status === "cancelled"
      ? { status: "cancelled", taskId: event.taskId, phase: event.phase }
      : failedState(event.taskId, event.phase);
  }
  return event.status === "failed"
    ? {
        status: "failed",
        taskId: event.taskId,
        phase: event.phase,
        errorCode: safeImportErrorCode(event.error),
      }
    : failedState(event.taskId, event.phase);
}
