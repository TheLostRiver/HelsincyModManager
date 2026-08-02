import { useCallback, useRef, useState } from "react";
import {
  getBatchModLifecycleResult,
  previewBatchModLifecycle,
  retryBatchModLifecycle,
  sealBatchModLifecycle,
  startBatchModLifecycle,
} from "./batchModLifecycleApi.ts";
import type {
  BatchModLifecycleExecutionPolicy,
  BatchModLifecycleRequestDto,
} from "./batchModLifecycleTypes.ts";
import {
  buildBatchModLifecycleRequest,
  type BatchModLifecycleItemResolution,
  type BatchModLifecycleOperation,
  type BatchModLifecycleWorkflowState,
  resolveBatchModLifecycleItems,
} from "./batchModLifecycleWorkflow.ts";
import type { InstallManifestStatusSummary } from "../modInstallPlanTypes.ts";
import type { ModRevisionList } from "../modLibraryTypes.ts";

export const DEFAULT_BATCH_EXECUTION_POLICY: BatchModLifecycleExecutionPolicy =
  "stop_on_failure";
const BATCH_RESULT_PAGE_LIMIT = 50;

export type UseBatchModLifecycleWorkflowInput = {
  gameId: string | null;
  profileId: string | null;
  loadManifestStatuses: (modIds: string[]) => Promise<InstallManifestStatusSummary[]>;
  loadRevisions: (modId: string) => Promise<ModRevisionList>;
};

function preferBatchRevision(revisions: ModRevisionList): string | null {
  if (revisions.displayRevisionId.length > 0) {
    return revisions.displayRevisionId;
  }
  return revisions.revisions[0]?.revisionId ?? null;
}

function operationOfRequest(request: BatchModLifecycleRequestDto): BatchModLifecycleOperation {
  return request.operation as BatchModLifecycleOperation;
}

export function commandErrorCode(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    const code = (error as { code?: unknown }).code;
    if (typeof code === "string" && code.length > 0) {
      return code;
    }
  }
  if (typeof error === "string" && error.length > 0) {
    return error;
  }
  return "batch_internal_error";
}

export function useBatchModLifecycleWorkflow(input: UseBatchModLifecycleWorkflowInput) {
  const { gameId, profileId, loadManifestStatuses, loadRevisions } = input;
  const [state, setState] = useState<BatchModLifecycleWorkflowState>({ status: "idle" });
  const stateRef = useRef(state);
  const generationRef = useRef(0);
  const resolutionRef = useRef<BatchModLifecycleItemResolution>({
    items: [],
    excluded: [],
    unresolvable: [],
  });
  const activeAttemptRef = useRef<{ batchId: string; attemptNumber: number } | null>(null);

  const updateState = useCallback((next: BatchModLifecycleWorkflowState) => {
    stateRef.current = next;
    setState(next);
  }, []);

  const requestFor = useCallback(
    (operation: BatchModLifecycleOperation, policy: BatchModLifecycleExecutionPolicy) => {
      if (gameId === null || profileId === null) {
        return null;
      }
      return buildBatchModLifecycleRequest({
        operation,
        gameId,
        profileId,
        policy,
        items: resolutionRef.current.items,
      });
    },
    [gameId, profileId],
  );

  const loadResultPage = useCallback(
    async (
      batchId: string,
      attemptNumber: number,
      cursor: string | null,
      generation: number,
      operation: BatchModLifecycleOperation,
    ) => {
      try {
        const result = await getBatchModLifecycleResult({
          batchId,
          attemptNumber,
          cursor,
          limit: BATCH_RESULT_PAGE_LIMIT,
        });
        if (generation !== generationRef.current) {
          return;
        }
        updateState({ status: "result", batchId, attemptNumber, operation, result });
      } catch (error) {
        if (generation === generationRef.current) {
          updateState({
            status: "result-error",
            errorCode: commandErrorCode(error),
            batchId,
            attemptNumber,
          });
        }
      }
    },
    [updateState],
  );

  const previewRequest = useCallback(
    async (
      request: BatchModLifecycleRequestDto,
      generation: number,
      policy: BatchModLifecycleExecutionPolicy,
    ) => {
      updateState({ status: "preview-loading", policy });
      try {
        const preview = await previewBatchModLifecycle(request);
        if (generation !== generationRef.current) {
          return;
        }
        updateState({ status: "preview-ready", request, preview, policy });
      } catch (error) {
        if (generation !== generationRef.current) {
          return;
        }
        updateState({
          status: "preview-error",
          errorCode: commandErrorCode(error),
          policy,
        });
      }
    },
    [updateState],
  );

  const prepare = useCallback(
    async (operation: BatchModLifecycleOperation, selectedModIds: string[]) => {
      if (gameId === null || profileId === null) {
        return;
      }
      if (selectedModIds.length === 0) {
        updateState({ status: "idle" });
        return;
      }
      const generation = ++generationRef.current;
      activeAttemptRef.current = null;
      updateState({ status: "resolving" });
      try {
        const manifestStatuses = await loadManifestStatuses(selectedModIds);
        const revisionsByMod: Record<string, ModRevisionList> = {};
        await Promise.all(
          selectedModIds.map(async (modId) => {
            try {
              revisionsByMod[modId] = await loadRevisions(modId);
            } catch {
              revisionsByMod[modId] = {
                modId,
                originRevisionId: "",
                displayRevisionId: "",
                revisions: [],
              };
            }
          }),
        );
        if (generation !== generationRef.current) {
          return;
        }
        const resolution = resolveBatchModLifecycleItems({
          operation,
          selectedModIds,
          manifestStatuses,
          revisionsByMod,
          preferRevision: preferBatchRevision,
        });
        resolutionRef.current = resolution;
        if (resolution.items.length === 0) {
          updateState({
            status: "preview-error",
            errorCode: "batch_no_applicable_items",
            policy: DEFAULT_BATCH_EXECUTION_POLICY,
          });
          return;
        }
        const request = requestFor(operation, DEFAULT_BATCH_EXECUTION_POLICY);
        if (request === null) {
          return;
        }
        await previewRequest(request, generation, DEFAULT_BATCH_EXECUTION_POLICY);
      } catch {
        if (generation === generationRef.current) {
          updateState({
            status: "preview-error",
            errorCode: "batch_facts_unavailable",
            policy: DEFAULT_BATCH_EXECUTION_POLICY,
          });
        }
      }
    },
    [gameId, profileId, loadManifestStatuses, loadRevisions, previewRequest, requestFor, updateState],
  );

  const setPolicy = useCallback(
    (policy: BatchModLifecycleExecutionPolicy) => {
      const current = stateRef.current;
      if (current.status !== "preview-ready" && current.status !== "preview-error") {
        return;
      }
      const generation = ++generationRef.current;
      const operation =
        current.status === "preview-ready"
          ? operationOfRequest(current.request)
          : "install";
      const request = requestFor(operation, policy);
      if (request === null) {
        return;
      }
      void previewRequest(request, generation, policy);
    },
    [previewRequest, requestFor],
  );

  const confirmAndStart = useCallback(async () => {
    const current = stateRef.current;
    if (current.status !== "preview-ready") {
      return;
    }
    const generation = generationRef.current;
    const operation = operationOfRequest(current.request);
    updateState({
      status: "confirming",
      request: current.request,
      preview: current.preview,
      policy: current.policy,
    });
    try {
      const sealed = await sealBatchModLifecycle({
        request: current.request,
        previewToken: current.preview.previewToken ?? "",
      });
      if (generation !== generationRef.current) {
        return;
      }
      updateState({
        status: "starting",
        request: current.request,
        preview: current.preview,
        policy: current.policy,
        batchId: sealed.batchId,
        planToken: sealed.planToken,
      });
      const started = await startBatchModLifecycle({
        batchId: sealed.batchId,
        planToken: sealed.planToken,
      });
      if (generation !== generationRef.current) {
        return;
      }
      activeAttemptRef.current = {
        batchId: started.batchId,
        attemptNumber: started.attemptNumber,
      };
      await loadResultPage(started.batchId, started.attemptNumber, null, generation, operation);
    } catch (error) {
      if (generation !== generationRef.current) {
        return;
      }
      updateState({
        status: "result-error",
        errorCode: commandErrorCode(error),
        batchId: activeAttemptRef.current?.batchId ?? null,
        attemptNumber: activeAttemptRef.current?.attemptNumber ?? null,
      });
    }
  }, [loadResultPage, updateState]);

  const retry = useCallback(async () => {
    const active = activeAttemptRef.current;
    const current = stateRef.current;
    if (active === null) {
      return;
    }
    const operation = current.status === "result" ? current.operation : "install";
    const generation = ++generationRef.current;
    try {
      const started = await retryBatchModLifecycle({
        batchId: active.batchId,
        expectedAttemptNumber: active.attemptNumber,
      });
      if (generation !== generationRef.current) {
        return;
      }
      activeAttemptRef.current = {
        batchId: started.batchId,
        attemptNumber: started.attemptNumber,
      };
      await loadResultPage(started.batchId, started.attemptNumber, null, generation, operation);
    } catch (error) {
      if (generation === generationRef.current) {
        updateState({
          status: "result-error",
          errorCode: commandErrorCode(error),
          batchId: active.batchId,
          attemptNumber: active.attemptNumber,
        });
      }
    }
  }, [loadResultPage, updateState]);

  const loadMoreResult = useCallback(async () => {
    const active = activeAttemptRef.current;
    const current = stateRef.current;
    if (active === null || current.status !== "result" || current.result.nextCursor === null) {
      return;
    }
    const generation = generationRef.current;
    try {
      const page = await getBatchModLifecycleResult({
        batchId: active.batchId,
        attemptNumber: active.attemptNumber,
        cursor: current.result.nextCursor,
        limit: BATCH_RESULT_PAGE_LIMIT,
      });
      if (generation !== generationRef.current) {
        return;
      }
      updateState({
        status: "result",
        batchId: current.batchId,
        attemptNumber: current.attemptNumber,
        operation: current.operation,
        result: {
          ...page,
          items: [...current.result.items, ...page.items],
        },
      });
    } catch (error) {
      if (generation === generationRef.current) {
        updateState({
          status: "result-error",
          errorCode: commandErrorCode(error),
          batchId: active.batchId,
          attemptNumber: active.attemptNumber,
        });
      }
    }
  }, [updateState]);

  const reset = useCallback(() => {
    generationRef.current += 1;
    activeAttemptRef.current = null;
    resolutionRef.current = { items: [], excluded: [], unresolvable: [] };
    updateState({ status: "idle" });
  }, [updateState]);

  return {
    state,
    resolution: resolutionRef.current,
    prepare,
    setPolicy,
    confirmAndStart,
    retry,
    loadMoreResult,
    reset,
  };
}
