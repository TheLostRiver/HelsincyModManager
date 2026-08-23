import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import { useCallback, useEffect, useRef, useState } from "react";
import { listCategories, type CategoryItem } from "../../categories/categoryApi";
import {
  createExternalImportSelection,
  getExternalImportPreview,
  selectAllExternalImportCandidates,
  startExternalImportBatch,
  updateExternalImportSelection,
} from "./externalImportApi";
import {
  appendExternalImportPreviewCandidates,
  isExternalImportPreviewPageForBatch,
  toExternalImportPreviewCandidateViewModel,
  type ExternalImportPreviewCandidateViewModel,
} from "./externalImportPreviewModel";
import type { ExternalImportTaskState } from "./externalImportProgressState";
import {
  applyExternalImportSelectionMutationResult,
  canSelectExternalImportCandidateWithDecision,
  getExternalImportSelectionErrorMessage,
  isExternalImportSelectionCategory,
  isExternalImportSelectionDto,
  isExternalImportSelectionExpired,
  isExternalImportSelectionMutationResultDto,
} from "./externalImportSelectionModel";
import {
  type ExternalImportSelectionDecisionDto,
  type ExternalImportSelectionDto,
} from "./externalImportTypes";
import {
  useExternalImportTaskProgress,
  type ExternalImportListenerStatus,
} from "./useExternalImportTaskProgress";
import {
  useExternalImportResultWorkflow,
  type ExternalImportResultWorkflow,
} from "./useExternalImportResultWorkflow";

export type ExternalImportSelectionPreviewState =
  | { status: "idle" }
  | { status: "loading" }
  | {
      status: "ready";
      candidates: ExternalImportPreviewCandidateViewModel[];
      totalCount: number;
      nextCursor: string | null;
      loadingMore: boolean;
      loadMoreError: string | null;
    }
  | { status: "empty"; totalCount: number }
  | { status: "failed"; message: string };

export type ExternalImportCategoryState =
  | { status: "loading"; options: CategoryItem[] }
  | { status: "ready"; options: CategoryItem[] }
  | { status: "failed"; options: CategoryItem[]; message: string };

type PendingSelectionAction = null | "select-all" | "start" | { candidateId: string };

export type ExternalImportSelectionWorkflow = {
  previewState: ExternalImportSelectionPreviewState;
  selection: ExternalImportSelectionDto | null;
  categoryState: ExternalImportCategoryState;
  decisionDrafts: Readonly<Record<string, ExternalImportSelectionDecisionDto>>;
  selectionError: string | null;
  pendingAction: PendingSelectionAction;
  importState: ExternalImportTaskState;
  listenerStatus: ExternalImportListenerStatus;
  cancelPending: boolean;
  selectionEditable: boolean;
  importActive: boolean;
  result: ExternalImportResultWorkflow;
  loadMore: () => void;
  retryPreview: () => void;
  retryCategories: () => void;
  retryListener: () => void;
  setCandidateDecision: (
    candidateId: string,
    decision: ExternalImportSelectionDecisionDto,
  ) => void;
  setCandidateSelected: (candidateId: string, selected: boolean) => void;
  selectAll: () => void;
  startImport: () => void;
  cancelImport: () => void;
};

const emptyDecision: ExternalImportSelectionDecisionDto = {
  conflictResolution: null,
  categoryId: null,
};

function errorCodeFrom(error: unknown, fallback: string) {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string"
  ) {
    return error.code;
  }
  return fallback;
}

function decisionForCandidate(candidate: ExternalImportPreviewCandidateViewModel) {
  return candidate.selectionDecision ?? emptyDecision;
}

export function useExternalImportSelectionWorkflow(
  batchId: string | null,
  onImported: () => Promise<void> | void,
): ExternalImportSelectionWorkflow {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const {
    importState,
    listenerStatus,
    cancelPending,
    importActive,
    isImportActive,
    launchImport,
    retryListener,
    cancelImport,
  } = useExternalImportTaskProgress(batchId);
  const [previewState, setPreviewState] =
    useState<ExternalImportSelectionPreviewState>({ status: "idle" });
  const previewStateRef = useRef<ExternalImportSelectionPreviewState>(previewState);
  const [selection, setSelection] = useState<ExternalImportSelectionDto | null>(null);
  const selectionRef = useRef<ExternalImportSelectionDto | null>(selection);
  const [categoryState, setCategoryState] = useState<ExternalImportCategoryState>({
    status: "loading",
    options: [],
  });
  const categoryStateRef = useRef<ExternalImportCategoryState>(categoryState);
  const [decisionDrafts, setDecisionDrafts] = useState<
    Record<string, ExternalImportSelectionDecisionDto>
  >({});
  const decisionDraftsRef = useRef(decisionDrafts);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<PendingSelectionAction>(null);
  const pendingActionRef = useRef<PendingSelectionAction>(pendingAction);
  const batchIdRef = useRef<string | null>(batchId);
  const workflowGenerationRef = useRef(0);
  const previewRequestRef = useRef(0);
  const categoryRequestRef = useRef(0);
  batchIdRef.current = batchId;
  const resultWorkflow = useExternalImportResultWorkflow({
    batchId,
    selectionId: selection?.selectionId ?? null,
    importState,
    importActive,
    progressReady: listenerStatus === "ready",
    launchImport,
    onImported,
  });

  const setTrackedPreviewState = useCallback(
    (
      update:
        | ExternalImportSelectionPreviewState
        | ((
            current: ExternalImportSelectionPreviewState,
          ) => ExternalImportSelectionPreviewState),
    ) => {
      const next = typeof update === "function" ? update(previewStateRef.current) : update;
      previewStateRef.current = next;
      setPreviewState(next);
    },
    [],
  );

  const setTrackedSelection = useCallback(
    (next: ExternalImportSelectionDto | null) => {
      selectionRef.current = next;
      setSelection(next);
    },
    [],
  );

  const setTrackedCategoryState = useCallback((next: ExternalImportCategoryState) => {
    categoryStateRef.current = next;
    setCategoryState(next);
  }, []);

  const setTrackedDecisionDrafts = useCallback(
    (
      update:
        | Record<string, ExternalImportSelectionDecisionDto>
        | ((
            current: Record<string, ExternalImportSelectionDecisionDto>,
          ) => Record<string, ExternalImportSelectionDecisionDto>),
    ) => {
      const next = typeof update === "function" ? update(decisionDraftsRef.current) : update;
      decisionDraftsRef.current = next;
      setDecisionDrafts(next);
    },
    [],
  );

  const setTrackedPendingAction = useCallback((next: PendingSelectionAction) => {
    pendingActionRef.current = next;
    setPendingAction(next);
  }, []);

  const isCurrentSelectionWorkflow = useCallback(
    (
      expectedBatchId: string,
      expectedSelectionId: string,
      expectedGeneration: number,
    ) =>
      batchIdRef.current === expectedBatchId &&
      workflowGenerationRef.current === expectedGeneration &&
      selectionRef.current?.selectionId === expectedSelectionId,
    [],
  );

  const reconcileDecisionDrafts = useCallback(
    (
      candidates: ExternalImportPreviewCandidateViewModel[],
      resetAuthoritative: boolean,
    ) => {
      setTrackedDecisionDrafts((current) => {
        const next = resetAuthoritative ? {} : { ...current };
        for (const candidate of candidates) {
          if (candidate.selected || !(candidate.candidateId in next)) {
            next[candidate.candidateId] = decisionForCandidate(candidate);
          }
        }
        return next;
      });
    },
    [setTrackedDecisionDrafts],
  );

  const loadFirstPage = useCallback(
    async (
      expectedBatchId: string,
      selectionId: string,
      options: { showLoading: boolean; resetDrafts: boolean },
    ) => {
      const requestId = previewRequestRef.current + 1;
      previewRequestRef.current = requestId;
      if (options.showLoading) {
        setTrackedPreviewState({ status: "loading" });
      }

      try {
        const page = await getExternalImportPreview({
          batchId: expectedBatchId,
          selectionId,
          cursor: null,
        });
        if (
          !isExternalImportPreviewPageForBatch(page, expectedBatchId, selectionId) ||
          page.selection === null
        ) {
          throw { code: "external_import_preview_invalid" };
        }
        if (
          previewRequestRef.current !== requestId ||
          batchIdRef.current !== expectedBatchId
        ) {
          return false;
        }

        const candidates = page.candidates.map((item) =>
        toExternalImportPreviewCandidateViewModel(item, extCopy.preview),
      );
        setTrackedSelection(page.selection);
        reconcileDecisionDrafts(candidates, options.resetDrafts);
        setTrackedPreviewState(
          candidates.length === 0
            ? { status: "empty", totalCount: page.totalCount }
            : {
                status: "ready",
                candidates,
                totalCount: page.totalCount,
                nextCursor: page.nextCursor,
                loadingMore: false,
                loadMoreError: null,
              },
        );
        return true;
      } catch (error) {
        if (
          previewRequestRef.current !== requestId ||
          batchIdRef.current !== expectedBatchId
        ) {
          return false;
        }
        setTrackedPreviewState({
          status: "failed",
          message: getExternalImportSelectionErrorMessage(
            errorCodeFrom(error, "external_import_preview_invalid"),
            extCopy.selection,
          ),
        });
        return false;
      }
    },
    [extCopy, reconcileDecisionDrafts, setTrackedPreviewState, setTrackedSelection],
  );

  const initializeSelection = useCallback(
    async (expectedBatchId: string, generation: number) => {
      setTrackedPreviewState({ status: "loading" });
      setSelectionError(null);
      try {
        const created = await createExternalImportSelection({ batchId: expectedBatchId });
        if (!isExternalImportSelectionDto(created) || created.status !== "editing") {
          throw { code: "external_import_selection_invalid" };
        }
        if (
          workflowGenerationRef.current !== generation ||
          batchIdRef.current !== expectedBatchId
        ) {
          return;
        }
        setTrackedSelection(created);
        await loadFirstPage(expectedBatchId, created.selectionId, {
          showLoading: false,
          resetDrafts: true,
        });
      } catch (error) {
        if (
          workflowGenerationRef.current !== generation ||
          batchIdRef.current !== expectedBatchId
        ) {
          return;
        }
        setTrackedPreviewState({
          status: "failed",
          message: getExternalImportSelectionErrorMessage(
            errorCodeFrom(error, "external_import_selection_invalid"),
            extCopy.selection,
          ),
        });
      }
    },
    [extCopy, loadFirstPage, setTrackedPreviewState, setTrackedSelection],
  );

  const loadCategoryOptions = useCallback(
    async (generation = workflowGenerationRef.current) => {
      const requestId = categoryRequestRef.current + 1;
      categoryRequestRef.current = requestId;
      setTrackedCategoryState({ status: "loading", options: [] });
      try {
        const categories = await listCategories();
        if (
          !Array.isArray(categories) ||
          !categories.every(isExternalImportSelectionCategory)
        ) {
          throw new Error("external_import_category_unavailable");
        }
        if (
          categoryRequestRef.current !== requestId ||
          workflowGenerationRef.current !== generation
        ) {
          return;
        }
        setTrackedCategoryState({ status: "ready", options: categories });
      } catch {
        if (
          categoryRequestRef.current !== requestId ||
          workflowGenerationRef.current !== generation
        ) {
          return;
        }
        setTrackedCategoryState({
          status: "failed",
          options: [],
          message: getExternalImportSelectionErrorMessage(
            "external_import_category_unavailable",
            extCopy.selection,
          ),
        });
      }
    },
    [extCopy, setTrackedCategoryState],
  );

  useEffect(() => {
    const generation = workflowGenerationRef.current + 1;
    workflowGenerationRef.current = generation;
    previewRequestRef.current += 1;
    categoryRequestRef.current += 1;
    setTrackedSelection(null);
    setTrackedDecisionDrafts({});
    setTrackedPendingAction(null);
    setSelectionError(null);

    if (batchId === null) {
      setTrackedPreviewState({ status: "idle" });
      setTrackedCategoryState({ status: "loading", options: [] });
      return;
    }

    void initializeSelection(batchId, generation);
    void loadCategoryOptions(generation);
  }, [
    batchId,
    initializeSelection,
    loadCategoryOptions,
    setTrackedCategoryState,
    setTrackedDecisionDrafts,
    setTrackedPendingAction,
    setTrackedPreviewState,
    setTrackedSelection,
  ]);

  useEffect(() => {
    if (selection === null || selection.status !== "editing") {
      return;
    }

    const expectedSelectionId = selection.selectionId;
    const expectedRevision = selection.revision;
    const markExpired = () => {
      const current = selectionRef.current;
      if (
        current?.selectionId === expectedSelectionId &&
        current.revision === expectedRevision &&
        isExternalImportSelectionExpired(current, Date.now())
      ) {
        setTrackedSelection({ ...current, status: "expired" });
      }
    };
    const delay = selection.expiresAtUnixMillis - Date.now();
    if (delay <= 0) {
      markExpired();
      return;
    }

    const timeoutId = window.setTimeout(markExpired, delay);
    return () => window.clearTimeout(timeoutId);
  }, [selection, setTrackedSelection]);

  const reloadAuthoritativeFirstPage = useCallback(async () => {
    const currentBatchId = batchIdRef.current;
    const currentSelection = selectionRef.current;
    if (currentBatchId === null || currentSelection === null) {
      return false;
    }
    return loadFirstPage(currentBatchId, currentSelection.selectionId, {
      showLoading: false,
      resetDrafts: false,
    });
  }, [loadFirstPage]);

  const mutateCandidate = useCallback(
    async (
      candidateId: string,
      selected: boolean,
      explicitDecision?: ExternalImportSelectionDecisionDto,
    ) => {
      const currentSelection = selectionRef.current;
      const currentPreview = previewStateRef.current;
      const currentBatchId = batchIdRef.current;
      const generation = workflowGenerationRef.current;
      if (
        pendingActionRef.current !== null ||
        currentBatchId === null ||
        currentSelection === null ||
        currentSelection.status !== "editing" ||
        currentPreview.status !== "ready" ||
        currentPreview.loadingMore
      ) {
        return;
      }

      const candidate = currentPreview.candidates.find(
        (item) => item.candidateId === candidateId,
      );
      if (!candidate) {
        return;
      }

      const decision = selected
        ? explicitDecision ?? decisionDraftsRef.current[candidateId] ?? emptyDecision
        : null;
      if (selected) {
        if (
          decision === null ||
          !canSelectExternalImportCandidateWithDecision(
            candidate.previewStatus,
            decision.conflictResolution,
          ) ||
          (decision.categoryId !== null &&
            (categoryStateRef.current.status !== "ready" ||
              !categoryStateRef.current.options.some(
                (category) => category.id === decision.categoryId,
              )))
        ) {
          setSelectionError(
            getExternalImportSelectionErrorMessage("selection_candidate_invalid", extCopy.selection),
          );
          return;
        }
      }

      setTrackedPendingAction({ candidateId });
      setSelectionError(null);
      try {
        const result = await updateExternalImportSelection({
          selectionId: currentSelection.selectionId,
          expectedRevision: currentSelection.revision,
          entries: [{ candidateId, selected, decision }],
        });
        if (!isExternalImportSelectionMutationResultDto(result)) {
          throw { code: "external_import_selection_invalid" };
        }
        if (
          !isCurrentSelectionWorkflow(
            currentBatchId,
            currentSelection.selectionId,
            generation,
          )
        ) {
          return;
        }

        setTrackedSelection(
          applyExternalImportSelectionMutationResult(currentSelection, result),
        );
        setTrackedPreviewState((current) =>
          current.status === "ready"
            ? {
                ...current,
                candidates: current.candidates.map((item) =>
                  item.candidateId === candidateId
                    ? {
                        ...item,
                        selected,
                        selectionDecision: selected ? decision : null,
                      }
                    : item,
                ),
              }
            : current,
        );
        if (selected && decision !== null) {
          setTrackedDecisionDrafts((current) => ({
            ...current,
            [candidateId]: decision,
          }));
        }
      } catch (error) {
        if (
          !isCurrentSelectionWorkflow(
            currentBatchId,
            currentSelection.selectionId,
            generation,
          )
        ) {
          return;
        }
        const code = errorCodeFrom(error, "external_import_selection_invalid");
        if (code === "selection_revision_conflict") {
          await reloadAuthoritativeFirstPage();
          if (
            !isCurrentSelectionWorkflow(
              currentBatchId,
              currentSelection.selectionId,
              generation,
            )
          ) {
            return;
          }
        }
        if (code === "selection_expired") {
          const latestSelection = selectionRef.current;
          if (latestSelection?.selectionId === currentSelection.selectionId) {
            setTrackedSelection({ ...latestSelection, status: "expired" });
          }
        }
        setSelectionError(getExternalImportSelectionErrorMessage(code, extCopy.selection));
      } finally {
        if (
          isCurrentSelectionWorkflow(
            currentBatchId,
            currentSelection.selectionId,
            generation,
          )
        ) {
          setTrackedPendingAction(null);
        }
      }
    },
    [
      extCopy,
      isCurrentSelectionWorkflow,
      reloadAuthoritativeFirstPage,
      setTrackedDecisionDrafts,
      setTrackedPendingAction,
      setTrackedPreviewState,
      setTrackedSelection,
    ],
  );

  const setCandidateDecision = useCallback(
    (candidateId: string, decision: ExternalImportSelectionDecisionDto) => {
      const currentPreview = previewStateRef.current;
      if (
        pendingActionRef.current !== null ||
        currentPreview.status !== "ready" ||
        currentPreview.loadingMore ||
        selectionRef.current?.status !== "editing"
      ) {
        return;
      }
      const candidate = currentPreview.candidates.find(
        (item) => item.candidateId === candidateId,
      );
      if (!candidate) {
        return;
      }
      if (candidate.selected) {
        void mutateCandidate(candidateId, true, decision);
        return;
      }
      setTrackedDecisionDrafts((current) => ({
        ...current,
        [candidateId]: decision,
      }));
      setSelectionError(null);
    },
    [mutateCandidate, setTrackedDecisionDrafts],
  );

  const setCandidateSelected = useCallback(
    (candidateId: string, selected: boolean) => {
      void mutateCandidate(candidateId, selected);
    },
    [mutateCandidate],
  );

  const selectAll = useCallback(async () => {
    const currentSelection = selectionRef.current;
    const currentPreview = previewStateRef.current;
    const currentBatchId = batchIdRef.current;
    const generation = workflowGenerationRef.current;
    if (
      pendingActionRef.current !== null ||
      currentBatchId === null ||
      currentSelection === null ||
      currentSelection.status !== "editing" ||
      currentPreview.status !== "ready" ||
      currentPreview.loadingMore
    ) {
      return;
    }

    setTrackedPendingAction("select-all");
    setSelectionError(null);
    try {
      const result = await selectAllExternalImportCandidates({
        selectionId: currentSelection.selectionId,
        expectedRevision: currentSelection.revision,
      });
      if (!isExternalImportSelectionMutationResultDto(result)) {
        throw { code: "external_import_selection_invalid" };
      }
      if (
        !isCurrentSelectionWorkflow(
          currentBatchId,
          currentSelection.selectionId,
          generation,
        )
      ) {
        return;
      }
      setTrackedSelection(
        applyExternalImportSelectionMutationResult(currentSelection, result),
      );
      await reloadAuthoritativeFirstPage();
    } catch (error) {
      if (
        !isCurrentSelectionWorkflow(
          currentBatchId,
          currentSelection.selectionId,
          generation,
        )
      ) {
        return;
      }
      const code = errorCodeFrom(error, "external_import_selection_invalid");
      if (code === "selection_revision_conflict") {
        await reloadAuthoritativeFirstPage();
        if (
          !isCurrentSelectionWorkflow(
            currentBatchId,
            currentSelection.selectionId,
            generation,
          )
        ) {
          return;
        }
      }
      if (code === "selection_expired") {
        const latestSelection = selectionRef.current;
        if (latestSelection?.selectionId === currentSelection.selectionId) {
          setTrackedSelection({ ...latestSelection, status: "expired" });
        }
      }
      setSelectionError(getExternalImportSelectionErrorMessage(code, extCopy.selection));
    } finally {
      if (
        isCurrentSelectionWorkflow(
          currentBatchId,
          currentSelection.selectionId,
          generation,
        )
      ) {
        setTrackedPendingAction(null);
      }
    }
  }, [
    extCopy,
    isCurrentSelectionWorkflow,
    reloadAuthoritativeFirstPage,
    setTrackedPendingAction,
    setTrackedSelection,
  ]);

  const startImport = useCallback(async () => {
    const currentBatchId = batchIdRef.current;
    const currentSelection = selectionRef.current;
    const generation = workflowGenerationRef.current;
    if (
      currentSelection !== null &&
      isExternalImportSelectionExpired(currentSelection, Date.now())
    ) {
      setTrackedSelection({ ...currentSelection, status: "expired" });
      setSelectionError(
        getExternalImportSelectionErrorMessage("selection_expired", extCopy.selection),
      );
      return;
    }
    if (
      listenerStatus !== "ready" ||
      pendingActionRef.current !== null ||
      currentBatchId === null ||
      currentSelection === null ||
      currentSelection.status !== "editing" ||
      currentSelection.selectedCount === 0 ||
      isImportActive() ||
      previewStateRef.current.status !== "ready" ||
      previewStateRef.current.loadingMore
    ) {
      return;
    }

    setTrackedPendingAction("start");
    setSelectionError(null);
    try {
      const launchResult = await launchImport(() =>
        startExternalImportBatch({
          batchId: currentBatchId,
          selectionId: currentSelection.selectionId,
          expectedRevision: currentSelection.revision,
        }),
      );
      if (
        !isCurrentSelectionWorkflow(
          currentBatchId,
          currentSelection.selectionId,
          generation,
        )
      ) {
        return;
      }
      if (launchResult.status === "started") {
        setTrackedSelection({ ...currentSelection, status: "sealed" });
      } else if (launchResult.status === "failed") {
        setSelectionError(
          getExternalImportSelectionErrorMessage(launchResult.errorCode, extCopy.selection),
        );
      } else if (launchResult.status === "ignored") {
        setSelectionError(
          getExternalImportSelectionErrorMessage(
            "external_import_task_unavailable",
            extCopy.selection,
          ),
        );
      }
    } catch (error) {
      if (
        !isCurrentSelectionWorkflow(
          currentBatchId,
          currentSelection.selectionId,
          generation,
        )
      ) {
        return;
      }
      const code = errorCodeFrom(error, "external_import_task_unavailable");
      setSelectionError(getExternalImportSelectionErrorMessage(code, extCopy.selection));
    } finally {
      if (
        isCurrentSelectionWorkflow(
          currentBatchId,
          currentSelection.selectionId,
          generation,
        )
      ) {
        setTrackedPendingAction(null);
      }
    }
  }, [
    extCopy,
    isCurrentSelectionWorkflow,
    setTrackedPendingAction,
    setTrackedSelection,
    isImportActive,
    launchImport,
    listenerStatus,
  ]);

  const loadMore = useCallback(async () => {
    const currentBatchId = batchIdRef.current;
    const currentSelection = selectionRef.current;
    const currentPreview = previewStateRef.current;
    if (
      currentBatchId === null ||
      currentSelection === null ||
      currentSelection.status !== "editing" ||
      pendingActionRef.current !== null ||
      isImportActive() ||
      currentPreview.status !== "ready" ||
      currentPreview.nextCursor === null ||
      currentPreview.loadingMore
    ) {
      return;
    }

    const requestId = previewRequestRef.current + 1;
    previewRequestRef.current = requestId;
    setTrackedPreviewState({ ...currentPreview, loadingMore: true, loadMoreError: null });
    try {
      const page = await getExternalImportPreview({
        batchId: currentBatchId,
        selectionId: currentSelection.selectionId,
        cursor: currentPreview.nextCursor,
      });
      if (
        !isExternalImportPreviewPageForBatch(
          page,
          currentBatchId,
          currentSelection.selectionId,
        ) ||
        page.selection === null
      ) {
        throw { code: "external_import_preview_invalid" };
      }
      if (
        previewRequestRef.current !== requestId ||
        batchIdRef.current !== currentBatchId
      ) {
        return;
      }

      const incoming = page.candidates.map((item) =>
        toExternalImportPreviewCandidateViewModel(item, extCopy.preview),
      );
      const candidates = appendExternalImportPreviewCandidates(
        currentPreview.candidates,
        page.candidates,
        extCopy.preview,
      );
      setTrackedSelection(page.selection);
      reconcileDecisionDrafts(incoming, false);
      setTrackedPreviewState({
        status: "ready",
        candidates,
        totalCount: page.totalCount,
        nextCursor: page.nextCursor,
        loadingMore: false,
        loadMoreError: null,
      });
    } catch (error) {
      if (
        previewRequestRef.current !== requestId ||
        batchIdRef.current !== currentBatchId
      ) {
        return;
      }
      setTrackedPreviewState({
        ...currentPreview,
        loadingMore: false,
        loadMoreError: getExternalImportSelectionErrorMessage(
          errorCodeFrom(error, "external_import_preview_invalid"),
          extCopy.selection,
        ),
      });
    }
  }, [
    extCopy,
    reconcileDecisionDrafts,
    isImportActive,
    setTrackedPreviewState,
    setTrackedSelection,
  ]);

  const retryPreview = useCallback(() => {
    const currentBatchId = batchIdRef.current;
    const currentSelection = selectionRef.current;
    if (currentBatchId === null) {
      return;
    }
    if (currentSelection === null) {
      void initializeSelection(currentBatchId, workflowGenerationRef.current);
      return;
    }
    void loadFirstPage(currentBatchId, currentSelection.selectionId, {
      showLoading: true,
      resetDrafts: false,
    });
  }, [initializeSelection, loadFirstPage]);

  const retryCategories = useCallback(() => {
    void loadCategoryOptions();
  }, [loadCategoryOptions]);

  const runLoadMore = useCallback(() => {
    void loadMore();
  }, [loadMore]);

  const runSelectAll = useCallback(() => {
    void selectAll();
  }, [selectAll]);

  const runStartImport = useCallback(() => {
    void startImport();
  }, [startImport]);

  const selectionEditable =
    selection !== null &&
    selection.status === "editing" &&
    !isExternalImportSelectionExpired(selection, Date.now()) &&
    !importActive &&
    !(previewState.status === "ready" && previewState.loadingMore);

  return {
    previewState,
    selection,
    categoryState,
    decisionDrafts,
    selectionError,
    pendingAction,
    importState,
    listenerStatus,
    cancelPending,
    selectionEditable,
    importActive,
    result: resultWorkflow,
    loadMore: runLoadMore,
    retryPreview,
    retryCategories,
    retryListener,
    setCandidateDecision,
    setCandidateSelected,
    selectAll: runSelectAll,
    startImport: runStartImport,
    cancelImport,
  };
}
