import {
  failedMessageKindFrom,
  getModImportFailedMessage,
  getModImportArchiveKeptMessage,
  type ModImportFailedMessageKind,
  type ModImportTerminalState,
} from "./modImportTaskState.ts";
import type { ModImportCopy } from "./modImportCopy";

/** 后端只读预检的逐条结果；前端不重复判断归档格式或内容根。 */
export type DroppedArchivePreview = {
  archivePath: string;
  fileName: string;
  sizeBytes: number | null;
  errorCode: string | null;
  warningCode: string | null;
};

export type DropRowWarningKind = "no-game-content";
const warningKindByCode: Readonly<Record<string, DropRowWarningKind>> = {
  mod_import_archive_no_game_content: "no-game-content",
};

/** 内容警示仍可手动勾选，不能等同于物理上无法导入的 blocked。 */
export type DropRowStatus = "importable" | "warned" | "blocked";
export type DropRowPhase = "pending" | "queued" | "running" | "succeeded" | "failed" | "cancelled" | "skipped";

export type DropRow = {
  archivePath: string;
  fileName: string;
  sizeBytes: number | null;
  status: DropRowStatus;
  messageKind: ModImportFailedMessageKind | null;
  warningKind: DropRowWarningKind | null;
  selected: boolean;
  phase: DropRowPhase;
  outcome: ModImportTerminalState | null;
};

export function isDropRowSelectable(row: DropRow): boolean {
  return row.status !== "blocked" && row.phase === "pending";
}

function rowFromPreview(preview: DroppedArchivePreview): DropRow {
  if (preview.errorCode !== null) {
    return {
      archivePath: preview.archivePath,
      fileName: preview.fileName,
      sizeBytes: preview.sizeBytes,
      status: "blocked",
      messageKind: failedMessageKindFrom(preview.errorCode),
      warningKind: null,
      selected: false,
      phase: "pending",
      outcome: null,
    };
  }
  const warningKind = preview.warningCode === null ? null : warningKindByCode[preview.warningCode] ?? null;
  return {
    archivePath: preview.archivePath,
    fileName: preview.fileName,
    sizeBytes: preview.sizeBytes,
    status: warningKind === null ? "importable" : "warned",
    messageKind: null,
    warningKind,
    selected: warningKind === null,
    phase: "pending",
    outcome: null,
  };
}

export function dropRowsFromPreviews(previews: readonly DroppedArchivePreview[]): DropRow[] {
  return previews.map(rowFromPreview);
}

/** 明确全选可覆盖内容警示，不能勾选阻断项或已提交项。 */
export function setAllDropRowsSelected(rows: readonly DropRow[], selected: boolean): DropRow[] {
  return rows.map((row) => isDropRowSelectable(row) ? { ...row, selected } : row);
}

export function selectedDropRows(rows: readonly DropRow[]): DropRow[] {
  return rows.filter((row) => isDropRowSelectable(row) && row.selected);
}

export function selectableDropRowCount(rows: readonly DropRow[]): number {
  return rows.filter(isDropRowSelectable).length;
}

export function dropSelectAllState(rows: readonly DropRow[]): "none" | "some" | "all" {
  const selectable = rows.filter(isDropRowSelectable);
  if (selectable.length === 0) return "none";
  const selected = selectable.filter((row) => row.selected).length;
  return selected === 0 ? "none" : selected === selectable.length ? "all" : "some";
}

export function canStartDropImport(rows: readonly DropRow[]): boolean {
  return selectedDropRows(rows).length > 0;
}

export type DropQueueSummary = {
  pending: number;
  queued: number;
  running: number;
  succeeded: number;
  failed: number;
  cancelled: number;
  skipped: number;
  /** 只计实际提交的条目，跳过项不改变执行进度分母。 */
  submitted: number;
  active: boolean;
};

export function dropQueueSummary(rows: readonly DropRow[]): DropQueueSummary {
  const count = (phase: DropRowPhase) => rows.filter((row) => row.phase === phase).length;
  const queued = count("queued");
  const running = count("running");
  const succeeded = count("succeeded");
  const failed = count("failed");
  const cancelled = count("cancelled");
  return {
    pending: count("pending"), queued, running, succeeded, failed, cancelled, skipped: count("skipped"),
    submitted: queued + running + succeeded + failed + cancelled,
    active: queued + running > 0,
  };
}

export function getDropRowNote(row: DropRow, copy: ModImportCopy): string | null {
  if (row.outcome?.status === "failed") return getModImportFailedMessage(row.outcome.messageKind, copy);
  if (row.outcome?.status === "cancelled") return copy.status.cancelled;
  if (row.phase === "cancelled") return copy.drop.cancelledBeforeStart;
  if (row.outcome?.status === "completed" && row.outcome.archiveKept !== null) {
    return getModImportArchiveKeptMessage(row.outcome.archiveKept, copy);
  }
  if (row.messageKind !== null) return getModImportFailedMessage(row.messageKind, copy);
  if (row.warningKind === "no-game-content") return copy.drop.warnNoGameContent;
  return null;
}

export function getDropQueueStatus(summary: DropQueueSummary, copy: ModImportCopy): string | null {
  if (summary.running > 0) return copy.drop.running(summary.succeeded + summary.failed + summary.cancelled + 1, summary.submitted);
  if (summary.queued > 0) return copy.drop.waitingCount(summary.queued);
  if (summary.submitted === 0) return null;
  return copy.drop.batchResult(summary.succeeded, summary.failed, summary.cancelled, summary.skipped);
}

/** 单次请求上限与后端一致；超限整批拒绝，不截断用户选择。 */
export const MAX_DROPPED_ARCHIVES = 100;

export function dedupeDroppedPaths(paths: readonly string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const path of paths) {
    if (path.length > 0 && !seen.has(path)) {
      seen.add(path);
      result.push(path);
    }
  }
  return result;
}
