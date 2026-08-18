import type { TaskStartedDto } from "../modImportTypes";

export const BATCH_MOD_LIFECYCLE_SCHEMA_VERSION = 1;
export const BATCH_MOD_LIFECYCLE_RESULT_PAGE_SIZE = 50;
export const BATCH_MOD_LIFECYCLE_RESULT_PAGE_MAX_SIZE = 100;
export const BATCH_MOD_LIFECYCLE_MAX_ITEMS = 100;

export type BatchModLifecycleOperation = "install" | "uninstall" | "reinstall";

export type BatchModLifecycleExecutionPolicy =
  | "stop_on_failure"
  | "continue_on_item_failure";

export type BatchModLifecycleLayerDto = {
  name: string;
  priority: number;
};

export type BatchModLifecycleItemInputDto =
  | {
      operation: "install";
      modId: string;
      revisionId: string;
      layer: BatchModLifecycleLayerDto;
    }
  | {
      operation: "uninstall";
      modId: string;
      expectedInstalledRevisionId: string;
    }
  | {
      operation: "reinstall";
      modId: string;
      installedRevisionId: string;
      candidateRevisionId: string;
      layer: BatchModLifecycleLayerDto;
    };

export type BatchModLifecycleReplacementTargetDto = {
  modId: string;
  targetId: string;
};

export type BatchModLifecycleReplacementTargetOption = {
  id: string;
  displayName: string;
  secondaryName?: string;
};

export type BatchModLifecycleReplacementTargetFacts = {
  modId: string;
  retargetable: boolean;
  installedTargetId: string | null;
  targets: BatchModLifecycleReplacementTargetOption[];
};

export type BatchModLifecycleRequestDto = {
  schemaVersion: number;
  operation: BatchModLifecycleOperation;
  gameId: string;
  profileId: string;
  executionPolicy: BatchModLifecycleExecutionPolicy;
  items: BatchModLifecycleItemInputDto[];
  /** Same-revision reinstall target switches; omitted for install/uninstall. */
  replacementTargets?: BatchModLifecycleReplacementTargetDto[];
};

export type BatchModLifecyclePreviewStatus = "ready" | "blocked";

export type BatchModLifecycleReasonSummaryDto = {
  code: string;
  count: number;
};

export type BatchModLifecycleActionSummaryDto = {
  actions: number;
  retained: number;
  replaced: number;
  added: number;
  stale: number;
};

export type BatchModLifecyclePreviewDto = {
  status: BatchModLifecyclePreviewStatus;
  operation: BatchModLifecycleOperation;
  executionPolicy: BatchModLifecycleExecutionPolicy;
  itemReasons: BatchModLifecycleReasonSummaryDto[];
  globalReasons: BatchModLifecycleReasonSummaryDto[];
  actionSummary: BatchModLifecycleActionSummaryDto;
  readyItemCount: number;
  blockedItemCount: number;
  previewToken: string | null;
};

export type BatchModLifecycleSealDto = {
  batchId: string;
  status: "sealed";
  operation: BatchModLifecycleOperation;
  executionPolicy: BatchModLifecycleExecutionPolicy;
  expiresAtUnixMillis: number;
  planToken: string;
};

export type BatchModLifecycleCapabilityDto = {
  previewAvailable: boolean;
  writeAvailable: boolean;
  unavailableReasonCode: string | null;
};

export type BatchModLifecycleStartedDto = {
  task: TaskStartedDto;
  batchId: string;
  attemptNumber: number;
};

export type BatchModLifecycleAttemptStatus =
  | "sealed"
  | "queued"
  | "running"
  | "stopping"
  | "completed"
  | "completed_with_errors"
  | "blocked"
  | "cancelled"
  | "recovery_required"
  | "interrupted"
  | "failed";

export type BatchModLifecycleItemStatus =
  | "running"
  | "succeeded"
  | "blocked"
  | "failed"
  | "recovery_required"
  | "cancelled"
  | "skipped";

export type BatchModLifecycleResultSummaryDto = {
  itemCount: number;
  succeededCount: number;
  blockedCount: number;
  failedCount: number;
  cancelledCount: number;
  skippedCount: number;
  recoveryRequiredCount: number;
};

export type BatchModLifecycleResultItemDto = {
  itemId: string;
  ordinal: number;
  modId: string;
  status: BatchModLifecycleItemStatus;
  reasonCode: string | null;
  retryable: boolean;
};

export type BatchModLifecycleResultPageDto = {
  batchId: string;
  attemptNumber: number;
  status: BatchModLifecycleAttemptStatus;
  taskId: string | null;
  evidenceHealthDegraded: boolean;
  summary: BatchModLifecycleResultSummaryDto;
  items: BatchModLifecycleResultItemDto[];
  nextCursor: string | null;
};

const BATCH_MOD_LIFECYCLE_OPERATIONS = new Set<string>([
  "install",
  "uninstall",
  "reinstall",
]);

const BATCH_MOD_LIFECYCLE_POLICIES = new Set<string>([
  "stop_on_failure",
  "continue_on_item_failure",
]);

const BATCH_MOD_LIFECYCLE_ITEM_STATUSES = new Set<string>([
  "running",
  "succeeded",
  "blocked",
  "failed",
  "recovery_required",
  "cancelled",
  "skipped",
]);

const BATCH_MOD_LIFECYCLE_ATTEMPT_STATUSES = new Set<string>([
  "sealed",
  "queued",
  "running",
  "stopping",
  "completed",
  "completed_with_errors",
  "blocked",
  "cancelled",
  "recovery_required",
  "interrupted",
  "failed",
]);

export function isBatchModLifecycleOperation(
  value: unknown,
): value is BatchModLifecycleOperation {
  return typeof value === "string" && BATCH_MOD_LIFECYCLE_OPERATIONS.has(value);
}

export function isBatchModLifecycleExecutionPolicy(
  value: unknown,
): value is BatchModLifecycleExecutionPolicy {
  return typeof value === "string" && BATCH_MOD_LIFECYCLE_POLICIES.has(value);
}

export function isBatchModLifecycleItemStatus(
  value: unknown,
): value is BatchModLifecycleItemStatus {
  return typeof value === "string" && BATCH_MOD_LIFECYCLE_ITEM_STATUSES.has(value);
}

export function isBatchModLifecycleAttemptStatus(
  value: unknown,
): value is BatchModLifecycleAttemptStatus {
  return (
    typeof value === "string" && BATCH_MOD_LIFECYCLE_ATTEMPT_STATUSES.has(value)
  );
}
