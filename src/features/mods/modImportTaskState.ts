import type { ModImportCopy } from "./modImportCopy";
import type { TaskProgressEventDto } from "./modImportTypes";

export type ModImportTaskState =
  | { status: "idle" }
  | { status: "choosing" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: string }
  | { status: "completed"; taskId: string; phase: string; archiveKept: ModImportArchiveKeptCode | null }
  | { status: "cancelled"; taskId: string; phase: string }
  | { status: "failed"; taskId: string | null; phase: string; messageKind: ModImportFailedMessageKind };

export type ModImportTerminalState = Extract<ModImportTaskState, { status: "completed" | "cancelled" | "failed" }>;

export function modImportStartFailureKind(error: unknown): ModImportFailedMessageKind {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : null;
  if (code === "archive_path_empty" || code === "archive_path_not_absolute") return "invalid-archive";
  if (code === "mod_storage_migration_in_progress") return "storage-frozen-migration";
  if (code === "mod_storage_restart_required") return "storage-frozen-restart";
  if (code === "mod_import_preview_limit_exceeded") return "preview-limit";
  return "start-failed";
}

/**
 * #275 ④「移动导入」：导入已成功、只是源压缩包没删的降级码（契约「移动导入（#275 切片④）」）。
 * 挂在 completed 事件的 error 上；不在此列表的字串一律忽略，不当码用。
 */
export type ModImportArchiveKeptCode =
  | "mod_import_archive_kept_not_regular_file"
  | "mod_import_archive_kept_protected_location"
  | "mod_import_archive_kept_changed"
  | "mod_import_archive_kept_unavailable"
  | "mod_import_archive_kept_remove_failed";

const archiveKeptCodes: ReadonlySet<string> = new Set<ModImportArchiveKeptCode>([
  "mod_import_archive_kept_not_regular_file",
  "mod_import_archive_kept_protected_location",
  "mod_import_archive_kept_changed",
  "mod_import_archive_kept_unavailable",
  "mod_import_archive_kept_remove_failed",
]);

export function archiveKeptCodeFrom(error: string | null): ModImportArchiveKeptCode | null {
  return error !== null && archiveKeptCodes.has(error) ? (error as ModImportArchiveKeptCode) : null;
}

export function getModImportArchiveKeptMessage(code: ModImportArchiveKeptCode, copy: ModImportCopy): string {
  return copy.archiveKept[code];
}

// 失败原因只存语义，渲染时经 getModImportFailedMessage 按当前界面语言取词；
// 绝不把后端事件内容拼进用户可见消息（脱敏语义与语言无关）。
export type ModImportFailedMessageKind =
  | "retry-hint"
  | "listener-unavailable"
  | "picker-failed"
  | "invalid-start-state"
  | "invalid-archive"
  | "start-failed"
  | "storage-frozen-migration"
  | "storage-frozen-restart"
  | "unsupported-archive-format"
  | "not-an-archive"
  | "archive-encrypted"
  | "archive-multi-volume"
  | "preview-limit";

// 后端投影的解包失败语义码 -> 档位（#348）。
//
// 此前失败分支写死 retry-hint，`event.error` 根本没被读过，于是 `.rar` 只能显示
// 「请检查压缩包后重试」——包是好的，提示把玩家指向了错误的方向。
//
// 认不出的码一律落回 retry-hint：后端将来新增码时，前端最差也只是退回今天的行为，
// 不会白屏也不会显示空文案。
const failedMessageKindByErrorCode: ReadonlyMap<string, ModImportFailedMessageKind> = new Map([
  ["mod_import_unsupported_archive_format", "unsupported-archive-format"],
  ["mod_import_not_an_archive", "not-an-archive"],
  // 容器打得开、但用了我们不支持的特性（#348 切片 C）。与「格式不支持」分开，
  // 是因为玩家的下一步动作完全不同：那边要转档，这边要去掉密码 / 拿到完整分卷。
  ["mod_import_archive_encrypted", "archive-encrypted"],
  ["mod_import_archive_multi_volume", "archive-multi-volume"],
]);

export function failedMessageKindFrom(error: string | null): ModImportFailedMessageKind {
  if (error === null) return "retry-hint";
  return failedMessageKindByErrorCode.get(error) ?? "retry-hint";
}

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
    case "storage-frozen-migration":
      return copy.errors.storageFrozenMigration;
    case "storage-frozen-restart":
      return copy.errors.storageFrozenRestart;
    case "unsupported-archive-format":
      return copy.errors.unsupportedArchiveFormat;
    case "not-an-archive":
      return copy.errors.notAnArchive;
    case "archive-encrypted":
      return copy.errors.archiveEncrypted;
    case "archive-multi-volume":
      return copy.errors.archiveMultiVolume;
    case "preview-limit":
      return copy.errors.previewLimitExceeded;
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
    return {
      status: "completed",
      taskId: event.taskId,
      phase: event.phase,
      archiveKept: archiveKeptCodeFrom(event.error),
    };
  }
  if (event.status === "cancelled") {
    return { status: "cancelled", taskId: event.taskId, phase: event.phase };
  }
  if (event.status === "failed") {
    return {
      status: "failed",
      taskId: event.taskId,
      phase: event.phase,
      messageKind: failedMessageKindFrom(event.error),
    };
  }

  return { status: "running", taskId: event.taskId, phase: event.phase };
}
