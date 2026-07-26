import {
  EXTERNAL_IMPORT_RESULT_PAGE_MAX_SIZE,
  EXTERNAL_IMPORT_RESULT_TOTAL_MAX_SIZE,
  isExternalImportOpaqueId,
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

const itemStatusPresentation: Readonly<
  Record<
    ExternalImportItemStatus,
    Pick<ExternalImportResultViewModel, "statusLabel" | "statusTone">
  >
> = {
  imported: { statusLabel: "已导入", statusTone: "ready" },
  already_imported: { statusLabel: "已存在", statusTone: "neutral" },
  skipped: { statusLabel: "已跳过", statusTone: "neutral" },
  blocked: { statusLabel: "已阻断", statusTone: "danger" },
  failed: { statusLabel: "导入失败", statusTone: "danger" },
  cancelled: { statusLabel: "已取消", statusTone: "warning" },
};

const reasonLabels: Readonly<Record<string, string>> = {
  already_imported: "内容已存在",
  duplicate_in_batch: "批次内重复",
  name_collision: "名称冲突",
  structure_invalid: "目录结构不可用",
  metadata_invalid: "元数据不可用",
  unsupported_entry: "包含不支持的条目",
  resource_limit_exceeded: "超出资源限制",
  source_unreadable: "来源不可读取",
  source_changed: "来源已变化",
  selection_revision_conflict: "选择版本已变化",
  selection_empty: "选择为空",
  selection_mutation_empty: "没有选择变更",
  selection_mutation_limit_exceeded: "选择变更超限",
  selection_total_limit_exceeded: "选择总数超限",
  selection_resource_limit_exceeded: "选择资源超限",
  selection_candidate_invalid: "候选状态已变化",
  selection_expired: "选择已过期",
  selection_closed: "选择已封存",
  selection_revision_overflow: "选择版本不可用",
};

const batchStatusLabels: Readonly<Record<string, string>> = {
  completed: "全部完成",
  completed_with_errors: "部分完成",
  failed: "任务失败，已保留结果",
  cancelled: "任务已取消，已保留结果",
};

const resultErrorMessages: Readonly<Record<string, string>> = {
  external_import_batch_unavailable: "批量导入结果不可用，请重新扫描",
  external_import_batch_not_startable: "当前批次没有可重试项",
  external_import_result_cursor_invalid: "结果分页位置不可用，请重新载入",
  external_import_result_limit_invalid: "结果分页大小不可用，请重新载入",
  external_import_result_request_invalid: "结果请求不可用，请重新载入",
  external_import_selection_unavailable: "已封存的选择不可用，请重新扫描",
  external_import_source_unavailable: "来源已失效，请重新选择来源后重试",
  external_import_task_unavailable: "导入任务不可用，请稍后重试",
  external_import_result_invalid: "批量导入结果不可识别，请重新扫描",
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
): ExternalImportResultViewModel {
  const presentation = itemStatusPresentation[result.status];
  return {
    candidateId: result.candidateId,
    status: result.status,
    statusLabel: presentation.statusLabel,
    statusTone: presentation.statusTone,
    reasonLabel:
      result.reasonCode === null
        ? result.retryable
          ? "可重试"
          : null
        : reasonLabels[result.reasonCode] ?? "结果原因不可识别",
    importedModId: result.importedModId,
    retryable: result.retryable,
  };
}

export function appendExternalImportResults(
  existing: ExternalImportResultViewModel[],
  incoming: ExternalImportItemResultDto[],
) {
  const seenCandidateIds = new Set(existing.map((result) => result.candidateId));
  const next = [...existing];
  for (const result of incoming) {
    if (seenCandidateIds.has(result.candidateId)) {
      continue;
    }
    seenCandidateIds.add(result.candidateId);
    next.push(toExternalImportResultViewModel(result));
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
) {
  return batchStatusLabels[status] ?? "批次状态不可识别";
}

export function getExternalImportResultErrorMessage(errorCode: string) {
  return (
    resultErrorMessages[errorCode] ??
    "无法读取批量导入结果，请稍后重试"
  );
}
