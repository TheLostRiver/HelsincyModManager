import type { TaskProgressEventDto } from "../modImportTypes";

export type ExternalImportScanTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: string }
  | { status: "completed"; taskId: string; phase: string }
  | { status: "cancelled"; taskId: string; phase: string }
  | { status: "failed"; taskId: string | null; phase: string; errorCode: string };

import type { ExternalImportCopy } from "./externalImportCopy";

// 阶段/错误码集合只承担语义判断；文本一律经 copy 取。
const externalImportScanPhases: ReadonlySet<string> = new Set([
  "external_import.scan.queued",
  "external_import.scan.discovering",
  "external_import.scan.fingerprinting",
  "external_import.scan.completed",
  "external_import.scan.failed",
  "external_import.scan.cancelled",
  "mod_import.cancelled",
]);

export function isExternalImportScanPhase(phase: string) {
  return externalImportScanPhases.has(phase);
}

export function getExternalImportScanPhaseLabel(phase: string, scan: ExternalImportCopy["scan"]) {
  return scan.phases[phase] ?? scan.scanning;
}

export function getExternalImportScanErrorMessage(errorCode: string, scan: ExternalImportCopy["scan"]) {
  return scan.errors[errorCode] ?? scan.fallbackError;
}

export function isExternalImportScanTaskTerminal(state: ExternalImportScanTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

const knownScanErrorCodes: ReadonlySet<string> = new Set([
  "external_import_source_picker_unavailable",
  "external_import_source_unavailable",
  "external_import_source_id_invalid",
  "external_import_task_unavailable",
  "external_import_batch_unavailable",
  "external_import_scan_failed",
  "external_import_clock_unavailable",
  "external_import_preview_cursor_invalid",
  "external_import_preview_limit_invalid",
  "external_import_progress_unrecognized",
  "external_import_preview_invalid",
  "external_import_listener_unavailable",
]);

function toSafeExternalImportErrorCode(error: string | null) {
  if (error !== null && knownScanErrorCodes.has(error)) {
    return error;
  }
  return "external_import_scan_failed";
}

function failedScanState(
  taskId: string,
  phase: string,
  error: string | null,
): ExternalImportScanTaskState {
  return {
    status: "failed",
    taskId,
    phase,
    errorCode: toSafeExternalImportErrorCode(error),
  };
}

export function nextExternalImportScanTaskStateFromProgress(
  current: ExternalImportScanTaskState,
  event: TaskProgressEventDto,
): ExternalImportScanTaskState {
  if (isExternalImportScanTaskTerminal(current)) {
    return current;
  }

  if (
    event.kind !== "mod_import" ||
    !("taskId" in current) ||
    current.taskId !== event.taskId
  ) {
    return current;
  }

  if (!isExternalImportScanPhase(event.phase)) {
    return failedScanState(event.taskId, "external_import.scan.unrecognized", null);
  }

  if (event.phase === "mod_import.cancelled") {
    return event.status === "cancelled"
      ? { status: "cancelled", taskId: event.taskId, phase: event.phase }
      : failedScanState(event.taskId, event.phase, null);
  }

  if (
    event.phase === "external_import.scan.queued" ||
    event.phase === "external_import.scan.discovering" ||
    event.phase === "external_import.scan.fingerprinting"
  ) {
    return event.status === "queued" || event.status === "running"
      ? { status: "running", taskId: event.taskId, phase: event.phase }
      : failedScanState(event.taskId, event.phase, event.error);
  }

  if (event.phase === "external_import.scan.completed") {
    return event.status === "completed"
      ? { status: "completed", taskId: event.taskId, phase: event.phase }
      : failedScanState(event.taskId, event.phase, event.error);
  }

  if (event.phase === "external_import.scan.cancelled") {
    return event.status === "cancelled"
      ? { status: "cancelled", taskId: event.taskId, phase: event.phase }
      : failedScanState(event.taskId, event.phase, event.error);
  }

  return event.status === "failed"
    ? failedScanState(event.taskId, event.phase, event.error)
    : failedScanState(event.taskId, event.phase, null);
}
