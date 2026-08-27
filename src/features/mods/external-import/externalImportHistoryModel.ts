import {
  EXTERNAL_IMPORT_HISTORY_PAGE_MAX_SIZE,
  isExternalImportOpaqueId,
  isPlainRecord,
  isSafeNonNegativeInteger,
  type ExternalImportBatchImportStatus,
  type ExternalImportHistoryEntryDto,
  type ExternalImportHistoryPageDto,
  type ExternalImportScanStatus,
} from "./externalImportTypes.ts";

import type { ExternalImportCopy } from "./externalImportCopy";
// 值导入走具体模块而不是 i18n 目录桶:纯模型要能被 node --test 直接加载。
import { localeMeta, type Locale } from "../../../shared/i18n/locales.ts";

const scanStatuses = new Set(["pending", "running", "completed", "failed", "cancelled"]);
const importStatuses = new Set([
  "pending",
  "running",
  "completed",
  "completed_with_errors",
  "failed",
  "cancelled",
]);
const countKeys = [
  "total",
  "imported",
  "alreadyImported",
  "skipped",
  "blocked",
  "failed",
  "cancelled",
] as const;

export type ExternalImportHistoryStateTone =
  | "ready"
  | "warning"
  | "danger"
  | "neutral"
  | "progress";

export type ExternalImportHistoryStateKey =
  | "running"
  | "scanning"
  | "scanOnly"
  | "scanFailed"
  | "completed"
  | "completedWithErrors"
  | "incomplete"
  | "cancelled";

export type ExternalImportHistoryRowViewModel = {
  batchId: string;
  adapterLabel: string;
  createdAtLabel: string;
  stateKey: ExternalImportHistoryStateKey;
  stateLabel: string;
  stateTone: ExternalImportHistoryStateTone;
  total: number;
  imported: number;
  alreadyImported: number;
  skipped: number;
  blocked: number;
  failed: number;
  cancelled: number;
  candidateCount: number;
  hasDetails: boolean;
};

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]) {
  const keys = Object.keys(value);
  return keys.length === expected.length && keys.every((key) => expected.includes(key));
}

function isHistoryCounts(value: unknown): value is ExternalImportHistoryEntryDto["counts"] {
  if (!isPlainRecord(value) || !hasExactKeys(value, countKeys)) {
    return false;
  }
  if (!countKeys.every((key) => isSafeNonNegativeInteger(value[key]))) {
    return false;
  }
  // 计数必须能对上分项之和,这是「计数与明细同源」信任基础的客户端复核。
  const partsTotal =
    (value.imported as number) +
    (value.alreadyImported as number) +
    (value.skipped as number) +
    (value.blocked as number) +
    (value.failed as number) +
    (value.cancelled as number);
  return value.total === partsTotal;
}

function isHistoryEntry(value: unknown): value is ExternalImportHistoryEntryDto {
  if (
    !isPlainRecord(value) ||
    !hasExactKeys(value, [
      "batchId",
      "adapterId",
      "scanStatus",
      "importStatus",
      "createdAtUnixMillis",
      "candidateCount",
      "counts",
    ])
  ) {
    return false;
  }

  return (
    isExternalImportOpaqueId(value.batchId) &&
    isExternalImportOpaqueId(value.adapterId) &&
    typeof value.scanStatus === "string" &&
    scanStatuses.has(value.scanStatus) &&
    typeof value.importStatus === "string" &&
    importStatuses.has(value.importStatus) &&
    isSafeNonNegativeInteger(value.createdAtUnixMillis) &&
    isSafeNonNegativeInteger(value.candidateCount) &&
    isHistoryCounts(value.counts)
  );
}

function isValidHistoryNextCursor(value: unknown, totalCount: number, pageLength: number) {
  if (value === null) {
    return true;
  }
  if (typeof value !== "string" || !/^[1-9]\d*$/.test(value) || pageLength === 0) {
    return false;
  }
  const cursor = Number(value);
  return Number.isSafeInteger(cursor) && cursor < totalCount;
}

export function isExternalImportHistoryPage(
  value: unknown,
): value is ExternalImportHistoryPageDto {
  if (
    !isPlainRecord(value) ||
    !hasExactKeys(value, ["batches", "totalCount", "nextCursor"]) ||
    !Array.isArray(value.batches)
  ) {
    return false;
  }
  if (
    !isSafeNonNegativeInteger(value.totalCount) ||
    value.totalCount < value.batches.length ||
    // 后端的 COUNT(*) 与分页在同一事务内取,声明有批次却给空页只可能是契约漂移。
    // 不 fail closed 的话 refresh 会把它显示成「没有导入记录」,比报错更伤信任。
    (value.totalCount > 0 && value.batches.length === 0) ||
    value.batches.length > EXTERNAL_IMPORT_HISTORY_PAGE_MAX_SIZE ||
    !isValidHistoryNextCursor(value.nextCursor, value.totalCount, value.batches.length)
  ) {
    return false;
  }

  const batchIds = new Set<string>();
  for (const entry of value.batches) {
    if (!isHistoryEntry(entry) || batchIds.has(entry.batchId)) {
      return false;
    }
    batchIds.add(entry.batchId);
  }
  return true;
}

// 单一状态语义:scan/import 两个状态派生成用户可读的一个词。启动恢复会把中断的
// running 收敛为 failed,历史里对 failed 统一用中性「未完成」表述。
export function resolveExternalImportHistoryState(
  scanStatus: ExternalImportScanStatus,
  importStatus: ExternalImportBatchImportStatus,
): { key: ExternalImportHistoryStateKey; tone: ExternalImportHistoryStateTone } {
  switch (importStatus) {
    case "running":
      return { key: "running", tone: "progress" };
    case "completed":
      return { key: "completed", tone: "ready" };
    case "completed_with_errors":
      return { key: "completedWithErrors", tone: "warning" };
    case "failed":
      return { key: "incomplete", tone: "warning" };
    case "cancelled":
      return { key: "cancelled", tone: "neutral" };
    case "pending":
      break;
  }
  switch (scanStatus) {
    case "completed":
      return { key: "scanOnly", tone: "neutral" };
    case "failed":
      return { key: "scanFailed", tone: "danger" };
    case "cancelled":
      return { key: "cancelled", tone: "neutral" };
    case "pending":
    case "running":
      return { key: "scanning", tone: "progress" };
  }
}

const relativeWindowMillis = 7 * 24 * 60 * 60 * 1000;

export function formatExternalImportHistoryTime(
  createdAtUnixMillis: number,
  nowUnixMillis: number,
  timeCopy: ExternalImportCopy["history"]["time"],
  locale: Locale,
): string {
  const elapsed = Math.max(0, nowUnixMillis - createdAtUnixMillis);
  if (elapsed >= relativeWindowMillis) {
    return new Date(createdAtUnixMillis).toLocaleString(localeMeta[locale].bcp47, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
  if (elapsed < 60_000) {
    return timeCopy.justNow;
  }
  if (elapsed < 60 * 60_000) {
    return timeCopy.minutesAgo(String(Math.floor(elapsed / 60_000)));
  }
  if (elapsed < 24 * 60 * 60_000) {
    return timeCopy.hoursAgo(String(Math.floor(elapsed / (60 * 60_000))));
  }
  return timeCopy.daysAgo(String(Math.floor(elapsed / (24 * 60 * 60_000))));
}

export function toExternalImportHistoryRow(
  entry: ExternalImportHistoryEntryDto,
  historyCopy: ExternalImportCopy["history"],
  locale: Locale,
  nowUnixMillis: number,
): ExternalImportHistoryRowViewModel {
  const state = resolveExternalImportHistoryState(entry.scanStatus, entry.importStatus);
  return {
    batchId: entry.batchId,
    adapterLabel: historyCopy.adapters[entry.adapterId] ?? historyCopy.unknownAdapter,
    createdAtLabel: formatExternalImportHistoryTime(
      entry.createdAtUnixMillis,
      nowUnixMillis,
      historyCopy.time,
      locale,
    ),
    stateKey: state.key,
    stateLabel: historyCopy.states[state.key],
    stateTone: state.tone,
    total: entry.counts.total,
    imported: entry.counts.imported,
    alreadyImported: entry.counts.alreadyImported,
    skipped: entry.counts.skipped,
    blocked: entry.counts.blocked,
    failed: entry.counts.failed,
    cancelled: entry.counts.cancelled,
    candidateCount: entry.candidateCount,
    hasDetails: entry.counts.total > 0,
  };
}

export function appendExternalImportHistoryRows(
  existing: ExternalImportHistoryRowViewModel[],
  incoming: ExternalImportHistoryEntryDto[],
  historyCopy: ExternalImportCopy["history"],
  locale: Locale,
  nowUnixMillis: number,
): ExternalImportHistoryRowViewModel[] {
  const seenBatchIds = new Set(existing.map((row) => row.batchId));
  const next = [...existing];
  for (const entry of incoming) {
    if (seenBatchIds.has(entry.batchId)) {
      continue;
    }
    seenBatchIds.add(entry.batchId);
    next.push(toExternalImportHistoryRow(entry, historyCopy, locale, nowUnixMillis));
  }
  return next;
}

export function getExternalImportHistoryErrorMessage(
  errorCode: string,
  historyCopy: ExternalImportCopy["history"],
) {
  return historyCopy.errors[errorCode] ?? historyCopy.fallbackError;
}
