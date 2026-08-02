import type {
  BatchModLifecycleExecutionPolicy,
  BatchModLifecycleItemInputDto,
  BatchModLifecyclePreviewDto,
  BatchModLifecycleRequestDto,
  BatchModLifecycleResultPageDto,
  BatchModLifecycleStartedDto,
} from "./batchModLifecycleTypes.ts";
import { BATCH_MOD_LIFECYCLE_MAX_ITEMS, BATCH_MOD_LIFECYCLE_SCHEMA_VERSION } from "./batchModLifecycleTypes.ts";
import type { InstallManifestStatusSummary } from "../modInstallPlanTypes.ts";
import type { ModRevisionList } from "../modLibraryTypes.ts";

export const BATCH_BASE_LAYER = { name: "base", priority: 0 } as const;

export type BatchModLifecycleOperation = "install" | "uninstall" | "reinstall";

export type BatchModLifecycleItemFacts = {
  modId: string;
  /** Resolved source/candidate revision for install/reinstall; null when unavailable. */
  revisionId: string | null;
  /** Exact installed revision from manifest facts; null when not installed or legacy. */
  installedRevisionId: string | null;
  /** Why this mod is excluded from the current operation (stable reason, not UI copy). */
  excludedReason: string | null;
};

export type BatchModLifecycleWorkflowState =
  | { status: "idle" }
  | { status: "resolving" }
  | { status: "preview-loading"; policy: BatchModLifecycleExecutionPolicy }
  | {
      status: "preview-ready";
      request: BatchModLifecycleRequestDto;
      preview: BatchModLifecyclePreviewDto;
      policy: BatchModLifecycleExecutionPolicy;
    }
  | {
      status: "preview-error";
      errorCode: string;
      policy: BatchModLifecycleExecutionPolicy;
      operation: BatchModLifecycleOperation;
    }
  | { status: "confirming"; request: BatchModLifecycleRequestDto; preview: BatchModLifecyclePreviewDto; policy: BatchModLifecycleExecutionPolicy }
  | {
      status: "starting";
      request: BatchModLifecycleRequestDto;
      preview: BatchModLifecyclePreviewDto;
      policy: BatchModLifecycleExecutionPolicy;
      batchId: string;
      planToken: string;
    }
  | {
      status: "result";
      batchId: string;
      attemptNumber: number;
      operation: BatchModLifecycleOperation;
      result: BatchModLifecycleResultPageDto;
    }
  | { status: "result-error"; errorCode: string; batchId: string | null; attemptNumber: number | null };

export type BatchModLifecyclePreviewError = Extract<
  BatchModLifecycleWorkflowState,
  { status: "preview-error" }
>;

export type BatchModLifecycleItemResolution = {
  items: BatchModLifecycleItemInputDto[];
  /** Selected mods excluded from this operation, keyed by modId with a stable reason. */
  excluded: { modId: string; reason: string }[];
  /** Selected mods that failed to resolve (revision lookup unavailable). */
  unresolvable: string[];
};

export function resolveBatchModLifecycleItems(input: {
  operation: BatchModLifecycleOperation;
  selectedModIds: string[];
  manifestStatuses: InstallManifestStatusSummary[];
  revisionsByMod: Record<string, ModRevisionList>;
  preferRevision: (revisions: ModRevisionList) => string | null;
}): BatchModLifecycleItemResolution {
  const { operation, selectedModIds, manifestStatuses, revisionsByMod, preferRevision } = input;
  const statusByMod = new Map(manifestStatuses.map((status) => [status.modId, status]));
  const items: BatchModLifecycleItemInputDto[] = [];
  const excluded: { modId: string; reason: string }[] = [];
  const unresolvable: string[] = [];

  for (const modId of selectedModIds) {
    const status = statusByMod.get(modId);
    const installed = status?.status === "installed";
    const installedRevisionId = status?.installedRevisionId ?? null;

    if (operation === "install") {
      if (installed) {
        excluded.push({ modId, reason: "already_installed" });
        continue;
      }
      const revisions = revisionsByMod[modId] ?? [];
      const revisionId = preferRevision(revisions);
      if (revisionId === null) {
        unresolvable.push(modId);
        continue;
      }
      items.push({
        operation: "install",
        modId,
        revisionId,
        layer: { ...BATCH_BASE_LAYER },
      });
      continue;
    }

    if (operation === "uninstall" || operation === "reinstall") {
      if (!installed) {
        excluded.push({ modId, reason: "not_installed" });
        continue;
      }
      if (installedRevisionId === null) {
        excluded.push({ modId, reason: "installed_revision_unavailable" });
        continue;
      }
      if (operation === "uninstall") {
        items.push({
          operation: "uninstall",
          modId,
          expectedInstalledRevisionId: installedRevisionId,
        });
        continue;
      }
      const revisions = revisionsByMod[modId] ?? [];
      const candidateRevisionId = preferRevision(revisions);
      if (candidateRevisionId === null) {
        unresolvable.push(modId);
        continue;
      }
      items.push({
        operation: "reinstall",
        modId,
        installedRevisionId,
        candidateRevisionId,
        layer: { ...BATCH_BASE_LAYER },
      });
    }
  }

  return { items, excluded, unresolvable };
}

export function buildBatchModLifecycleRequest(input: {
  operation: BatchModLifecycleOperation;
  gameId: string;
  profileId: string;
  policy: BatchModLifecycleExecutionPolicy;
  items: BatchModLifecycleItemInputDto[];
}): BatchModLifecycleRequestDto {
  return {
    schemaVersion: BATCH_MOD_LIFECYCLE_SCHEMA_VERSION,
    operation: input.operation,
    gameId: input.gameId,
    profileId: input.profileId,
    executionPolicy: input.policy,
    items: input.items,
  };
}

export function batchModLifecycleRequestExceedsLimit(request: BatchModLifecycleRequestDto): boolean {
  return request.items.length > BATCH_MOD_LIFECYCLE_MAX_ITEMS;
}

/** Prefers the first listed revision, otherwise null. Callers decide the ordering policy. */
export function preferNewestBatchRevision(
  revisions: ReadonlyArray<{ revisionId: string }>,
): string | null {
  if (revisions.length === 0) {
    return null;
  }
  return revisions[0]?.revisionId ?? null;
}

export type BatchModLifecycleStartedProjection = {
  started: BatchModLifecycleStartedDto;
  operation: BatchModLifecycleOperation;
};

export function nextBatchModLifecycleResultCursor(
  result: BatchModLifecycleResultPageDto,
): string | null {
  return result.nextCursor;
}

export function batchTerminalAttemptCompleted(
  status: BatchModLifecycleResultPageDto["status"],
): boolean {
  return status === "completed" || status === "completed_with_errors";
}
