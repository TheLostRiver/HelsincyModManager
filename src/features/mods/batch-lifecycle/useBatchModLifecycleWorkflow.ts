import { useCallback, useEffect, useRef, useState } from "react";
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
  BatchModLifecycleReplacementTargetFacts,
} from "./batchModLifecycleTypes.ts";
import { BATCH_MOD_LIFECYCLE_MAX_ITEMS } from "./batchModLifecycleTypes.ts";
import {
  buildBatchModLifecycleRequest,
  type BatchModLifecycleItemResolution,
  type BatchModLifecycleOperation,
  type BatchModLifecycleWorkflowState,
  resolveBatchModLifecycleItems,
} from "./batchModLifecycleWorkflow.ts";
import type { InstallManifestStatusSummary } from "../modInstallPlanTypes.ts";
import type { ModRevisionList } from "../modLibraryTypes.ts";
import { selectionInput, setModPluginSelection } from "../../install-plugins/pluginSelectionApi.ts";
import type { PluginInventory } from "../../install-plugins/pluginSelectionTypes";
import { loadBatchPluginChoices } from "./batchPluginChoices.ts";

export const DEFAULT_BATCH_EXECUTION_POLICY: BatchModLifecycleExecutionPolicy =
  "stop_on_failure";
const BATCH_RESULT_PAGE_LIMIT = 50;

export type UseBatchModLifecycleWorkflowInput = {
  gameId: string | null;
  profileId: string | null;
  loadManifestStatuses: (modIds: string[]) => Promise<InstallManifestStatusSummary[]>;
  loadRevisions: (modId: string) => Promise<ModRevisionList>;
  loadReplacementTargetFacts?: (
    modIds: string[],
  ) => Promise<BatchModLifecycleReplacementTargetFacts[]>;
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
  const {
    gameId,
    profileId,
    loadManifestStatuses,
    loadRevisions,
    loadReplacementTargetFacts,
  } = input;
  const [state, setState] = useState<BatchModLifecycleWorkflowState>({ status: "idle" });
  const stateRef = useRef(state);
  const generationRef = useRef(0);
  const [pluginChoices, setPluginChoices] = useState<PluginInventory[]>([]);
  const [pluginSaving, setPluginSaving] = useState(false);
  const [pluginError, setPluginError] = useState<unknown>(null);
  const pluginSavingRef = useRef(false);
  const previewRequestRef = useRef<BatchModLifecycleRequestDto | null>(null);
  const resolutionRef = useRef<BatchModLifecycleItemResolution>({
    items: [],
    excluded: [],
    unresolvable: [],
  });
  const replacementTargetsRef = useRef<{ modId: string; targetId: string }[]>([]);
  const activeAttemptRef = useRef<{ batchId: string; attemptNumber: number } | null>(null);
  const scopeKey = JSON.stringify([gameId, profileId]);
  const stateScopeRef = useRef(scopeKey);

  const updateState = useCallback((next: BatchModLifecycleWorkflowState) => {
    stateRef.current = next;
    setState(next);
  }, []);

  useEffect(() => {
    stateScopeRef.current = scopeKey;
    generationRef.current += 1;
    pluginSavingRef.current = false;
    setPluginSaving(false);
    setPluginChoices([]);
    setPluginError(null);
    previewRequestRef.current = null;
    activeAttemptRef.current = null;
    replacementTargetsRef.current = [];
    resolutionRef.current = { items: [], excluded: [], unresolvable: [] };
    updateState({ status: "idle" });
    return () => { generationRef.current += 1; };
  }, [scopeKey, updateState]);

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
        replacementTargets: replacementTargetsRef.current,
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
      previewRequestRef.current = request;
      updateState({ status: "preview-loading", policy });
      try {
        const choices = await loadBatchPluginChoices(request, () => generation === generationRef.current);
        if (generation !== generationRef.current) return;
        setPluginChoices(choices);
        setPluginError(null);
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
          operation: operationOfRequest(request),
        });
      }
    },
    [updateState],
  );

  const changePlugin = async (inventory: PluginInventory, fileId: string, selected: boolean) => {
    const request = previewRequestRef.current;
    if (!request || pluginSavingRef.current || stateRef.current.status !== "preview-ready") return;
    const generation = ++generationRef.current;
    pluginSavingRef.current = true;
    setPluginSaving(true);
    setPluginError(null);
    updateState({ status: "preview-loading", policy: request.executionPolicy });
    try {
      await setModPluginSelection(selectionInput({ ...inventory, files: inventory.files.map((file) => file.fileId === fileId ? { ...file, selected } : file) }));
      if (generation === generationRef.current) await previewRequest(request, generation, request.executionPolicy);
    } catch (error) {
      if (generation === generationRef.current) {
        setPluginError(error);
        updateState({ status: "preview-error", errorCode: commandErrorCode(error), policy: request.executionPolicy, operation: operationOfRequest(request) });
      }
    } finally {
      if (generation === generationRef.current) {
        pluginSavingRef.current = false;
        setPluginSaving(false);
      }
    }
  };

  const reloadPlugins = () => {
    const request = previewRequestRef.current;
    if (request && !pluginSavingRef.current) void previewRequest(request, ++generationRef.current, request.executionPolicy);
  };

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
      setPluginChoices([]);
      setPluginError(null);
      previewRequestRef.current = null;
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
        replacementTargetsRef.current = [];
        if (resolution.items.length === 0) {
          updateState({
            status: "preview-error",
            errorCode: "batch_no_applicable_items",
            policy: DEFAULT_BATCH_EXECUTION_POLICY,
            operation,
          });
          return;
        }
        if (resolution.items.length > BATCH_MOD_LIFECYCLE_MAX_ITEMS) {
          updateState({
            status: "preview-error",
            errorCode: "batch_resource_limit_exceeded",
            policy: DEFAULT_BATCH_EXECUTION_POLICY,
            operation,
          });
          return;
        }

        const sameRevisionReinstallModIds =
          operation === "reinstall"
            ? resolution.items.flatMap((item) =>
                item.operation === "reinstall"
                && item.installedRevisionId === item.candidateRevisionId
                  ? [item.modId]
                  : [],
              )
            : [];
        if (sameRevisionReinstallModIds.length > 0) {
          if (loadReplacementTargetFacts === undefined) {
            updateState({
              status: "preview-error",
              errorCode: "batch_replacement_facts_unavailable",
              policy: DEFAULT_BATCH_EXECUTION_POLICY,
              operation,
            });
            return;
          }
          let targetFacts: BatchModLifecycleReplacementTargetFacts[];
          try {
            targetFacts = await loadReplacementTargetFacts(sameRevisionReinstallModIds);
          } catch {
            if (generation === generationRef.current) {
              updateState({
                status: "preview-error",
                errorCode: "batch_replacement_facts_unavailable",
                policy: DEFAULT_BATCH_EXECUTION_POLICY,
                operation,
              });
            }
            return;
          }
          if (generation !== generationRef.current) {
            return;
          }
          const factsByModId = new Map(targetFacts.map((facts) => [facts.modId, facts]));
          const orderedTargetFacts = sameRevisionReinstallModIds.map(
            (modId) =>
              factsByModId.get(modId) ?? {
                modId,
                retargetable: false,
                installedTargetId: null,
                targets: [],
              },
          );
          updateState({
            status: "target-selection",
            operation: "reinstall",
            policy: DEFAULT_BATCH_EXECUTION_POLICY,
            targetFacts: orderedTargetFacts,
            selectedTargets: Object.fromEntries(
              orderedTargetFacts.map((facts) => [facts.modId, null]),
            ),
            reapplyModIds: [],
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
            operation,
          });
        }
      }
    },
    [
      gameId,
      profileId,
      loadManifestStatuses,
      loadRevisions,
      loadReplacementTargetFacts,
      previewRequest,
      requestFor,
      updateState,
    ],
  );

  const setPolicy = useCallback(
    (policy: BatchModLifecycleExecutionPolicy) => {
      const current = stateRef.current;
      if (current.status === "target-selection") {
        updateState({ ...current, policy });
        return;
      }
      if (current.status !== "preview-ready" && current.status !== "preview-error") {
        return;
      }
      const generation = ++generationRef.current;
      const operation =
        current.status === "preview-ready"
          ? operationOfRequest(current.request)
          : current.operation;
      const request = requestFor(operation, policy);
      if (request === null) {
        return;
      }
      void previewRequest(request, generation, policy);
    },
    [previewRequest, requestFor, updateState],
  );

  const setReplacementTarget = useCallback(
    (modId: string, targetId: string) => {
      const current = stateRef.current;
      if (current.status !== "target-selection") {
        return;
      }
      const facts = current.targetFacts.find((candidate) => candidate.modId === modId);
      if (
        facts === undefined
        || !facts.retargetable
        || facts.installedTargetId === targetId
        || !facts.targets.some((target) => target.id === targetId)
      ) {
        return;
      }
      updateState({
        ...current,
        reapplyModIds: current.reapplyModIds.filter((id) => id !== modId),
        selectedTargets: {
          ...current.selectedTargets,
          [modId]: targetId,
        },
      });
    },
    [updateState],
  );

  const setReapplyTarget = useCallback((modId: string) => {
    const current = stateRef.current;
    if (current.status !== "target-selection" || !current.targetFacts.some((facts) => facts.modId === modId)) return;
    updateState({ ...current, selectedTargets: { ...current.selectedTargets, [modId]: null },
      reapplyModIds: [...current.reapplyModIds.filter((id) => id !== modId), modId] });
  }, [updateState]);

  const previewWithReplacementTargets = useCallback(() => {
    const current = stateRef.current;
    if (current.status !== "target-selection") {
      return;
    }
    const replacementTargets = current.targetFacts.filter((facts) => !current.reapplyModIds.includes(facts.modId)).map((facts) => ({
      facts,
      targetId: current.selectedTargets[facts.modId],
    }));
    if (
      replacementTargets.some(
        ({ facts, targetId }) =>
          !facts.retargetable
          || targetId === null
          || targetId === facts.installedTargetId
          || !facts.targets.some((target) => target.id === targetId),
      )
    ) {
      return;
    }
    replacementTargetsRef.current = replacementTargets.map(({ facts, targetId }) => ({
      modId: facts.modId,
      targetId: targetId as string,
    }));
    resolutionRef.current = { ...resolutionRef.current, items: resolutionRef.current.items.map((item) =>
      item.operation === "reinstall" && current.reapplyModIds.includes(item.modId)
        ? { ...item, intent: "reapply_equipment_targets" } : item) };
    const generation = ++generationRef.current;
    const request = requestFor("reinstall", current.policy);
    if (request === null) {
      return;
    }
    void previewRequest(request, generation, current.policy);
  }, [previewRequest, requestFor]);

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
      // Seal persists attempt 0; record it so a start failure keeps a recoverable identity.
      activeAttemptRef.current = {
        batchId: sealed.batchId,
        attemptNumber: 0,
      };
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

  const loadMorePendingRef = useRef(false);
  const loadMoreResult = useCallback(async () => {
    const active = activeAttemptRef.current;
    const current = stateRef.current;
    if (active === null || current.status !== "result" || current.result.nextCursor === null) {
      return;
    }
    if (loadMorePendingRef.current) {
      return;
    }
    loadMorePendingRef.current = true;
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
    } finally {
      loadMorePendingRef.current = false;
    }
  }, [updateState]);

  const reset = useCallback(() => {
    if (pluginSavingRef.current) return;
    generationRef.current += 1;
    setPluginChoices([]);
    setPluginError(null);
    previewRequestRef.current = null;
    activeAttemptRef.current = null;
    replacementTargetsRef.current = [];
    resolutionRef.current = { items: [], excluded: [], unresolvable: [] };
    updateState({ status: "idle" });
  }, [updateState]);

  return {
    pluginChoices, pluginSaving, pluginError, changePlugin, reloadPlugins,
    state: stateScopeRef.current === scopeKey ? state : { status: "idle" } as BatchModLifecycleWorkflowState,
    resolution: resolutionRef.current,
    prepare,
    setPolicy,
    setReplacementTarget,
    setReapplyTarget,
    previewWithReplacementTargets,
    confirmAndStart,
    retry,
    loadMoreResult,
    reset,
  };
}
