import type { ModImportCopy } from "./modImportCopy";
import type { TaskProgressEventDto } from "./modImportTypes";

export type ModImportTaskState =
  | { status: "idle" }
  | { status: "choosing" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: string }
  | { status: "completed"; taskId: string; phase: string }
  | { status: "cancelled"; taskId: string; phase: string }
  | { status: "failed"; taskId: string | null; phase: string; messageKind: ModImportFailedMessageKind };

// 失败原因只存语义，渲染时经 getModImportFailedMessage 按当前界面语言取词；
// 绝不把后端事件内容拼进用户可见消息（脱敏语义与语言无关）。
export type ModImportFailedMessageKind =
  | "retry-hint"
  | "listener-unavailable"
  | "picker-failed"
  | "invalid-start-state"
  | "invalid-archive"
  | "start-failed";

const modImportPhaseCopyKeys: Readonly<Record<string, keyof ModImportCopy["phases"]>> = {
  "mod_import.queued": "queued",
  "mod_import.cancelled": "cancelled",
  "mod_import.unpack.started": "unpackStarted",
  "mod_import.unpack.completed": "unpackCompleted",
  "mod_import.unpack.failed": "unpackFailed",
  "mod_import.preview_image.processing": "previewImageProcessing",
  "mod_import.preview_image.fallback": "previewImageFallback",
  "mod_import.analyze.processing": "analyzeProcessing",
  "mod_import.commit.processing": "commitProcessing",
  "mod_import.prepare.completed": "prepareCompleted",
};

export function isModImportTaskPhase(phase: string) {
  return Object.hasOwn(modImportPhaseCopyKeys, phase);
}

export function getModImportTaskPhaseLabel(phase: string, phases: ModImportCopy["phases"]) {
  const key = modImportPhaseCopyKeys[phase];
  return key === undefined ? phases.importing : phases[key];
}

export function getModImportFailedMessage(
  kind: ModImportFailedMessageKind,
  copy: ModImportCopy,
): string {
  switch (kind) {
    case "retry-hint":
      return copy.phases.failedRetryHint;
    case "listener-unavailable":
      return copy.status.unavailable;
    case "picker-failed":
      return copy.errors.pickerFailed;
    case "invalid-start-state":
      return copy.errors.invalidStartState;
    case "invalid-archive":
      return copy.errors.invalidArchive;
    case "start-failed":
      return copy.errors.startFailed;
  }
}

export function consumeReconnectImportRequest(
  listenerStatus: "loading" | "ready" | "failed",
  requested: boolean,
) {
  if (listenerStatus !== "ready" || !requested) {
    return { shouldStart: false, nextRequested: requested };
  }

  return { shouldStart: true, nextRequested: false };
}

export function nextModImportTaskStateFromProgress(
  current: ModImportTaskState,
  event: TaskProgressEventDto,
): ModImportTaskState {
  if (
    current.status === "completed" ||
    current.status === "cancelled" ||
    current.status === "failed"
  ) {
    return current;
  }

  if (
    event.kind !== "mod_import" ||
    !isModImportTaskPhase(event.phase) ||
    !("taskId" in current) ||
    current.taskId === null ||
    current.taskId !== event.taskId
  ) {
    return current;
  }

  if (event.status === "completed") {
    return { status: "completed", taskId: event.taskId, phase: event.phase };
  }
  if (event.status === "cancelled") {
    return { status: "cancelled", taskId: event.taskId, phase: event.phase };
  }
  if (event.status === "failed") {
    return {
      status: "failed",
      taskId: event.taskId,
      phase: event.phase,
      messageKind: "retry-hint",
    };
  }

  return { status: "running", taskId: event.taskId, phase: event.phase };
}
