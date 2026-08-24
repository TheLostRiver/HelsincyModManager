import {
  EXTERNAL_IMPORT_RESULT_PAGE_MAX_SIZE,
  EXTERNAL_IMPORT_RESULT_TOTAL_MAX_SIZE,
  isExternalImportOpaqueId,
  isOptionalDisplayText,
  isPlainRecord,
  isSafeNonNegativeInteger,
  type ExternalImportBatchImportStatus,
  type ExternalImportBatchResultPageDto,
  type ExternalImportItemResultDto,
  type ExternalImportItemStatus,
} from "./externalImportTypes.ts";

export const EXTERNAL_IMPORT_RESULT_10000_VALIDATION_BUDGET_MS = 250;

const terminalBatchStatuses = new Set([
  "completed",
  "completed_with_errors",
  "failed",
  "cancelled",
]);
const itemStatuses = new Set([
  "imported",
  "already_imported",
  "skipped",
  "blocked",
  "failed",
  "cancelled",
]);
const stableReasonCodes = new Set([
  "already_imported",
  "duplicate_in_batch",
  "name_collision",
  "structure_invalid",
  "metadata_invalid",
  "unsupported_entry",
  "resource_limit_exceeded",
  "source_unreadable",
  "source_changed",
  "selection_revision_conflict",
  "selection_empty",
  "selection_mutation_empty",
  "selection_mutation_limit_exceeded",
  "selection_total_limit_exceeded",
  "selection_resource_limit_exceeded",
  "selection_candidate_invalid",
  "selection_expired",
  "selection_closed",
  "selection_revision_overflow",
]);

type ResultTone = "ready" | "warning" | "danger" | "neutral";

export type ExternalImportResultViewModel = {
  candidateId: string;
  displayName: string | null;
  status: ExternalImportItemStatus;
  statusLabel: string;
  statusTone: ResultTone;
  reasonLabel: string | null;
  importedModId: string | null;
  retryable: boolean;
};

export type ExternalImportResultSummary = {
  imported: number;
  alreadyImported: number;
  skipped: number;
  blocked: number;
  failed: number;
  cancelled: number;
  retryable: number;
};

import type { ExternalImportCopy } from "./externalImportCopy";

// tone 是语义不是文案：文本经 copy.result.status 取，tone 固定在此。
const itemStatusTones: Readonly<
  Record<ExternalImportItemStatus, ExternalImportResultViewModel["statusTone"]>
> = {
  imported: "ready",
  already_imported: "neutral",
  skipped: "neutral",
  blocked: "danger",
  failed: "danger",
  cancelled: "warning",
};

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]) {
  const keys = Object.keys(value);
  return keys.length === expected.length && keys.every((key) => expected.includes(key));
}

function isResultItem(value: unknown): value is ExternalImportItemResultDto {
  if (
    !isPlainRecord(value) ||
    !hasExactKeys(value, [
      "candidateId",
      "displayName",
      "status",
      "reasonCode",
      "importedModId",
      "retryable",
    ])
  ) {
    return false;
  }

  return (
    isExternalImportOpaqueId(value.candidateId) &&
    isOptionalDisplayText(value.displayName) &&
    typeof value.status === "string" &&
    itemStatuses.has(value.status) &&
    (value.reasonCode === null ||
      (typeof value.reasonCode === "string" && stableReasonCodes.has(value.reasonCode))) &&
    (value.importedModId === null || isExternalImportOpaqueId(value.importedModId)) &&
    typeof value.retryable === "boolean"
  );
}

function isValidNextCursor(value: unknown, totalCount: number, pageLength: number) {
  if (value === null) {
    return true;
  }
  if (
    typeof value !== "string" ||
    !/^[1-9]\d*$/.test(value) ||
    pageLength === 0
  ) {
    return false;
  }
  const cursor = Number(value);
  return Number.isSafeInteger(cursor) && cursor < totalCount;
}

export function isExternalImportBatchResultPageForBatch(
  value: unknown,
  expectedBatchId: string,
): value is ExternalImportBatchResultPageDto {
  if (
    !isPlainRecord(value) ||
    !hasExactKeys(value, ["batch", "results", "totalCount", "nextCursor"]) ||
    !isPlainRecord(value.batch) ||
    !hasExactKeys(value.batch, ["batchId", "adapterId", "scanStatus", "importStatus"]) ||
    !Array.isArray(value.results)
  ) {
    return false;
  }

  const batch = value.batch;
  if (
    batch.batchId !== expectedBatchId ||
    !isExternalImportOpaqueId(batch.batchId) ||
    !isExternalImportOpaqueId(batch.adapterId) ||
    batch.scanStatus !== "completed" ||
    typeof batch.importStatus !== "string" ||
    !terminalBatchStatuses.has(batch.importStatus) ||
    !isSafeNonNegativeInteger(value.totalCount) ||
    value.totalCount < value.results.length ||
    value.totalCount > EXTERNAL_IMPORT_RESULT_TOTAL_MAX_SIZE ||
    (value.totalCount > 0 && value.results.length === 0) ||
    value.results.length > EXTERNAL_IMPORT_RESULT_PAGE_MAX_SIZE ||
    !isValidNextCursor(value.nextCursor, value.totalCount, value.results.length)
  ) {
    return false;
  }

  const candidateIds = new Set<string>();
  for (const result of value.results) {
    if (!isResultItem(result) || candidateIds.has(result.candidateId)) {
      return false;
    }
    candidateIds.add(result.candidateId);
  }
  return true;
}

export function toExternalImportResultViewModel(
  result: ExternalImportItemResultDto,
  resultCopy: ExternalImportCopy["result"],
): ExternalImportResultViewModel {
  // 守卫已拦截控制字符与超长;这里只把纯空白名归一为 null,由面板给未命名兜底文案。
  const displayName = result.displayName?.trim() || null;
  return {
    candidateId: result.candidateId,
    displayName,
    status: result.status,
    statusLabel: resultCopy.status[result.status] ?? resultCopy.unknownBatchStatus,
    statusTone: itemStatusTones[result.status],
    reasonLabel:
      result.reasonCode === null
        ? result.retryable
          ? resultCopy.retryable
          : null
        : resultCopy.reasons[result.reasonCode] ?? resultCopy.unknownReason,
    importedModId: result.importedModId,
    retryable: result.retryable,
  };
}

export function appendExternalImportResults(
  existing: ExternalImportResultViewModel[],
  incoming: ExternalImportItemResultDto[],
  resultCopy: ExternalImportCopy["result"],
) {
  const seenCandidateIds = new Set(existing.map((result) => result.candidateId));
  const next = [...existing];
  for (const result of incoming) {
    if (seenCandidateIds.has(result.candidateId)) {
      continue;
    }
    seenCandidateIds.add(result.candidateId);
    next.push(toExternalImportResultViewModel(result, resultCopy));
  }
  return next;
}

export function isExternalImportResultCoverageValid(
  totalCount: number,
  nextCursor: string | null,
  loadedCount: number,
) {
  if (
    !isSafeNonNegativeInteger(totalCount) ||
    !isSafeNonNegativeInteger(loadedCount) ||
    loadedCount > totalCount
  ) {
    return false;
  }
  return nextCursor === null
    ? loadedCount === totalCount
    : loadedCount < totalCount;
}

export function summarizeExternalImportResults(
  results: ExternalImportItemResultDto[] | ExternalImportResultViewModel[],
): ExternalImportResultSummary {
  const summary: ExternalImportResultSummary = {
    imported: 0,
    alreadyImported: 0,
    skipped: 0,
    blocked: 0,
    failed: 0,
    cancelled: 0,
    retryable: 0,
  };
  for (const result of results) {
    if (result.status === "already_imported") {
      summary.alreadyImported += 1;
    } else {
      summary[result.status] += 1;
    }
    if (result.retryable) {
      summary.retryable += 1;
    }
  }
  return summary;
}

export function getExternalImportBatchStatusLabel(
  status: ExternalImportBatchImportStatus,
  resultCopy: ExternalImportCopy["result"],
) {
  return resultCopy.batchStatus[status] ?? resultCopy.unknownBatchStatus;
}

export function getExternalImportResultErrorMessage(
  errorCode: string,
  resultCopy: ExternalImportCopy["result"],
) {
  return resultCopy.errors[errorCode] ?? resultCopy.fallbackError;
}
