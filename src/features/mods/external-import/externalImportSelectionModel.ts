import type { CategoryItem } from "../../categories/categoryApi";
import {
  isExternalImportDisplayText,
  isExternalImportOpaqueId,
  isPlainRecord,
  isSafeNonNegativeInteger,
  type ExternalImportBatchStartedDto,
  type ExternalImportCandidateStatus,
  type ExternalImportConflictResolution,
  type ExternalImportResourceUsageDto,
  type ExternalImportSelectionDecisionDto,
  type ExternalImportSelectionDto,
  type ExternalImportSelectionMutationResultDto,
} from "./externalImportTypes.ts";

const selectionStatuses = new Set(["editing", "sealed", "expired"]);
const conflictResolutions = new Set(["keep_both", "ignore_invalid_metadata"]);

const selectionErrorMessages: Readonly<Record<string, string>> = {
  external_import_selection_unavailable: "候选选择不可用，请重新扫描",
  external_import_batch_unavailable: "导入批次不可用，请重新扫描",
  external_import_batch_not_startable: "当前批次不能启动，请重新扫描",
  external_import_catalog_unavailable: "Mod 目录暂时不可用，请稍后重试",
  external_import_category_unavailable: "分类不可用，请重新载入分类",
  external_import_clock_unavailable: "选择状态不可用，请稍后重试",
  selection_revision_conflict: "选择已发生变化，已重新载入",
  selection_empty: "请至少选择一个候选",
  selection_mutation_empty: "没有需要更新的候选",
  selection_mutation_limit_exceeded: "本次选择变更过多，请分批操作",
  selection_total_limit_exceeded: "选择数量超出批次限制",
  selection_resource_limit_exceeded: "选择内容超出资源限制",
  selection_candidate_invalid: "候选状态已变化，请重新载入",
  selection_expired: "选择已过期，请重新扫描",
  selection_closed: "选择已封存，不能继续修改",
  external_import_selection_invalid: "选择数据不可识别，请重新扫描",
  external_import_task_unavailable: "导入任务不可用，请重试",
  external_import_progress_unrecognized: "导入状态不可识别，已停止继续操作",
};

function isResourceUsage(value: unknown): value is ExternalImportResourceUsageDto {
  return (
    isPlainRecord(value) &&
    isSafeNonNegativeInteger(value.fileCount) &&
    isSafeNonNegativeInteger(value.sourceBytes) &&
    isSafeNonNegativeInteger(value.materializationBytes)
  );
}

export function isExternalImportSelectionDecisionDto(
  value: unknown,
): value is ExternalImportSelectionDecisionDto {
  return (
    isPlainRecord(value) &&
    (value.conflictResolution === null ||
      (typeof value.conflictResolution === "string" &&
        conflictResolutions.has(value.conflictResolution))) &&
    (value.categoryId === null || isExternalImportOpaqueId(value.categoryId))
  );
}

export function isExternalImportSelectionDto(
  value: unknown,
): value is ExternalImportSelectionDto {
  return (
    isPlainRecord(value) &&
    isExternalImportOpaqueId(value.selectionId) &&
    isSafeNonNegativeInteger(value.revision) &&
    typeof value.status === "string" &&
    selectionStatuses.has(value.status) &&
    isSafeNonNegativeInteger(value.selectedCount) &&
    isResourceUsage(value.selectedResourceUsage) &&
    isSafeNonNegativeInteger(value.expiresAtUnixMillis)
  );
}

export function isExternalImportSelectionMutationResultDto(
  value: unknown,
): value is ExternalImportSelectionMutationResultDto {
  return (
    isPlainRecord(value) &&
    isSafeNonNegativeInteger(value.revision) &&
    isSafeNonNegativeInteger(value.selectedCount) &&
    isResourceUsage(value.selectedResourceUsage)
  );
}

export function applyExternalImportSelectionMutationResult(
  selection: ExternalImportSelectionDto,
  result: ExternalImportSelectionMutationResultDto,
): ExternalImportSelectionDto {
  return {
    ...selection,
    revision: result.revision,
    selectedCount: result.selectedCount,
    selectedResourceUsage: result.selectedResourceUsage,
  };
}

export function isExternalImportSelectionExpired(
  selection: ExternalImportSelectionDto,
  nowUnixMillis: number,
) {
  return (
    selection.status === "expired" ||
    (
      selection.status === "editing" &&
      selection.expiresAtUnixMillis <= nowUnixMillis
    )
  );
}

export function isExternalImportBatchStartedDto(
  value: unknown,
  expectedBatchId: string,
): value is ExternalImportBatchStartedDto {
  if (!isPlainRecord(value) || !isPlainRecord(value.task)) {
    return false;
  }

  return (
    value.batchId === expectedBatchId &&
    isExternalImportOpaqueId(value.batchId) &&
    isExternalImportOpaqueId(value.task.taskId) &&
    value.task.kind === "mod_import" &&
    value.task.status === "queued"
  );
}

export function getRequiredExternalImportConflictResolution(
  status: string,
): ExternalImportConflictResolution | null | "unsupported" {
  if (status === "ready") {
    return null;
  }
  if (status === "name_collision") {
    return "keep_both";
  }
  if (status === "metadata_invalid") {
    return "ignore_invalid_metadata";
  }
  return "unsupported";
}

export function canSelectExternalImportCandidateWithDecision(
  status: ExternalImportCandidateStatus | string,
  resolution: ExternalImportConflictResolution | null,
) {
  const required = getRequiredExternalImportConflictResolution(status);
  if (required === "unsupported") {
    return false;
  }
  if (required === null) {
    return resolution === null || resolution === "keep_both";
  }
  return resolution === required;
}

export function isExternalImportCandidateSelectionFactValid(
  status: ExternalImportCandidateStatus | string,
  selected: unknown,
  decision: unknown,
) {
  if (
    typeof selected !== "boolean" ||
    (decision !== null && !isExternalImportSelectionDecisionDto(decision))
  ) {
    return false;
  }
  if (!selected) {
    return decision === null;
  }

  return canSelectExternalImportCandidateWithDecision(
    status,
    decision?.conflictResolution ?? null,
  );
}

export function isExternalImportSelectionCategory(
  value: unknown,
): value is CategoryItem {
  if (!isPlainRecord(value)) {
    return false;
  }

  return (
    isExternalImportOpaqueId(value.id) &&
    isExternalImportDisplayText(value.name) &&
    value.name.trim().length > 0 &&
    Number.isSafeInteger(value.sortOrder) &&
    isSafeNonNegativeInteger(value.modCount) &&
    (value.color === undefined ||
      value.color === null ||
      isExternalImportDisplayText(value.color))
  );
}

export function getExternalImportSelectionErrorMessage(errorCode: string) {
  return (
    selectionErrorMessages[errorCode] ??
    "无法更新候选选择，请重新载入后重试"
  );
}
