import type { TaskProgressEventDto } from "../modImportTypes";

export type ExternalImportScanTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: string }
  | { status: "completed"; taskId: string; phase: string }
  | { status: "cancelled"; taskId: string; phase: string }
  | { status: "failed"; taskId: string | null; phase: string; errorCode: string };

const externalImportScanPhaseLabels: Readonly<Record<string, string>> = {
  "external_import.scan.queued": "等待只读扫描",
  "external_import.scan.discovering": "正在发现候选",
  "external_import.scan.fingerprinting": "正在分析候选",
  "external_import.scan.completed": "扫描完成",
  "external_import.scan.failed": "扫描失败",
  "external_import.scan.cancelled": "扫描已取消",
  "mod_import.cancelled": "正在取消扫描",
};

const externalImportScanErrorMessages: Readonly<Record<string, string>> = {
  external_import_source_picker_unavailable: "无法打开来源选择器",
  external_import_source_unavailable: "来源不可用，请重新选择",
  external_import_source_id_invalid: "来源标识无效，请重新选择",
  external_import_task_unavailable: "扫描任务不可用，请重新选择来源",
  external_import_batch_unavailable: "扫描预览不可用，请重新扫描",
  external_import_scan_failed: "扫描未完成，请重新选择来源后重试",
  external_import_clock_unavailable: "扫描状态不可用，请稍后重试",
  external_import_preview_cursor_invalid: "预览页状态无效，请重新扫描",
  external_import_preview_limit_invalid: "预览请求无效，请重新扫描",
  external_import_progress_unrecognized: "扫描状态不可识别，已停止继续操作",
  external_import_preview_invalid: "预览数据不可识别，请重新扫描",
  external_import_listener_unavailable: "扫描状态监听不可用，请重试",
};

export function isExternalImportScanPhase(phase: string) {
  return Object.hasOwn(externalImportScanPhaseLabels, phase);
}

export function getExternalImportScanPhaseLabel(phase: string) {
  return externalImportScanPhaseLabels[phase] ?? "正在扫描";
}

export function getExternalImportScanErrorMessage(errorCode: string) {
  return externalImportScanErrorMessages[errorCode] ?? "扫描未完成，请重新选择来源后重试";
}

export function isExternalImportScanTaskTerminal(state: ExternalImportScanTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

function toSafeExternalImportErrorCode(error: string | null) {
  if (
    error !== null &&
    Object.hasOwn(externalImportScanErrorMessages, error)
  ) {
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
