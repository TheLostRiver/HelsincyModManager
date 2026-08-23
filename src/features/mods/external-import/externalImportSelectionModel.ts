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

import type { ExternalImportCopy } from "./externalImportCopy";

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

export function getExternalImportSelectionErrorMessage(
  errorCode: string,
  selection: ExternalImportCopy["selection"],
) {
  return selection.errors[errorCode] ?? selection.fallbackError;
}
