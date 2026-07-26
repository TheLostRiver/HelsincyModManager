import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { listCategories, type CategoryItem } from "../../categories/categoryApi";
import { useFeedback } from "../../../shared/feedback";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../modImportTypes";
import {
  cancelExternalImportTask,
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
import {
  getExternalImportPhaseLabel,
  isExternalImportTaskTerminal,
  nextExternalImportTaskStateFromProgress,
  type ExternalImportTaskState,
} from "./externalImportProgressState";
import {
  applyExternalImportSelectionMutationResult,
  canSelectExternalImportCandidateWithDecision,
  getExternalImportSelectionErrorMessage,
  isExternalImportBatchStartedDto,
  isExternalImportSelectionCategory,
  isExternalImportSelectionDto,
  isExternalImportSelectionExpired,
  isExternalImportSelectionMutationResultDto,
} from "./externalImportSelectionModel";
import {
  type ExternalImportSelectionDecisionDto,
  type ExternalImportSelectionDto,
} from "./externalImportTypes";

type ListenerStatus = "loading" | "ready" | "failed";

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
  listenerStatus: ListenerStatus;
  cancelPending: boolean;
  selectionEditable: boolean;
  importActive: boolean;
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

function isImportActiveState(state: ExternalImportTaskState) {
  return (
    state.status === "starting" ||
    state.status === "running" ||
    state.status === "cancelling"
  );
}

export function useExternalImportSelectionWorkflow(
  batchId: string | null,
): ExternalImportSelectionWorkflow {
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();
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
  const [importState, setImportState] = useState<ExternalImportTaskState>({ status: "idle" });
  const importStateRef = useRef<ExternalImportTaskState>(importState);
  const [listenerStatus, setListenerStatus] = useState<ListenerStatus>("loading");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [cancelPending, setCancelPending] = useState(false);
  const cancelPendingRef = useRef(false);
  const batchIdRef = useRef<string | null>(batchId);
  const workflowGenerationRef = useRef(0);
  const previewRequestRef = useRef(0);
  const categoryRequestRef = useRef(0);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const displayedTaskNoticeIdRef = useRef<string | null>(null);
  const terminalNoticeKeysRef = useRef(new Set<string>());
  batchIdRef.current = batchId;

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

  const setTrackedImportState = useCallback((next: ExternalImportTaskState) => {
    importStateRef.current = next;
    setImportState(next);
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

        const candidates = page.candidates.map(toExternalImportPreviewCandidateViewModel);
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
          ),
        });
        return false;
      }
    },
    [reconcileDecisionDrafts, setTrackedPreviewState, setTrackedSelection],
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
          ),
        });
      }
    },
    [loadFirstPage, setTrackedPreviewState, setTrackedSelection],
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
          ),
        });
      }
    },
    [setTrackedCategoryState],
  );

  useEffect(() => {
    const generation = workflowGenerationRef.current + 1;
    workflowGenerationRef.current = generation;
    previewRequestRef.current += 1;
    categoryRequestRef.current += 1;
    taskIdRef.current = null;
    startPendingRef.current = false;
    pendingProgressEventsRef.current.clear();
    setTrackedSelection(null);
    setTrackedDecisionDrafts({});
    setTrackedPendingAction(null);
    setTrackedImportState({ status: "idle" });
    setSelectionError(null);
    cancelPendingRef.current = false;
    setCancelPending(false);

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
    setTrackedImportState,
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

  const applyProgressEvent = useCallback(
    (event: TaskProgressEventDto) => {
      const current = importStateRef.current;
      const next = nextExternalImportTaskStateFromProgress(current, event);
      if (next === current) {
        return;
      }
      if (isExternalImportTaskTerminal(next)) {
        taskIdRef.current = null;
      }
      setTrackedImportState(next);
    },
    [setTrackedImportState],
  );

  useEffect(() => {
    let disposed = false;
    let unlistenTaskProgress: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed || event.payload.kind !== "mod_import") {
        return;
      }

      const taskId = taskIdRef.current;
      if (taskId === null) {
        if (startPendingRef.current) {
          pendingProgressEventsRef.current.set(event.payload.taskId, event.payload);
        }
        return;
      }
      if (event.payload.taskId !== taskId) {
        return;
      }

      applyProgressEvent(event.payload);
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        unlistenTaskProgress = unlisten;
        setListenerStatus("ready");
      })
      .catch(() => {
        if (!disposed) {
          setListenerStatus("failed");
        }
      });

    return () => {
      disposed = true;
      unlistenTaskProgress?.();
    };
  }, [applyProgressEvent, listenerAttempt]);

  useEffect(() => {
    const previousTaskId = displayedTaskNoticeIdRef.current;
    if (importState.status === "running" || importState.status === "cancelling") {
      if (previousTaskId && previousTaskId !== importState.taskId) {
        dismissTaskNotice(previousTaskId);
      }
      displayedTaskNoticeIdRef.current = importState.taskId;
      const progress =
        importState.status === "running" &&
        importState.current !== null &&
        importState.total !== null
          ? `（${importState.current} / ${importState.total}）`
          : "";
      showTaskNotice({
        taskId: importState.taskId,
        title: "正在批量导入 Mod",
        message: `${getExternalImportPhaseLabel(importState.phase)}${progress}`,
        tone: "progress",
      });
      return;
    }

    if (previousTaskId) {
      dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = null;
    }
  }, [dismissTaskNotice, importState, showTaskNotice]);

  useEffect(
    () => () => {
      const taskId = displayedTaskNoticeIdRef.current;
      if (taskId) {
        dismissTaskNotice(taskId);
      }
    },
    [dismissTaskNotice],
  );

  useEffect(() => {
    if (!isExternalImportTaskTerminal(importState)) {
      return;
    }

    const noticeKey = `${importState.status}.${
      importState.taskId ?? `${batchIdRef.current ?? "no-batch"}.${importState.phase}`
    }`;
    if (terminalNoticeKeysRef.current.has(noticeKey)) {
      return;
    }
    terminalNoticeKeysRef.current.add(noticeKey);

    if (importState.status === "completed") {
      pushToast({
        eventKey: `external-import.import.completed.${noticeKey}`,
        taskId: importState.taskId,
        title: "批量导入已完成",
        message: "导入任务已完成；结果明细将在后续结果视图中提供。",
        tone: "success",
      });
      return;
    }

    if (importState.status === "cancelled") {
      pushToast({
        eventKey: `external-import.import.cancelled.${noticeKey}`,
        taskId: importState.taskId,
        title: "批量导入已取消",
        message: "已保留后端确认的结果；本页面不推断部分成功数量。",
        tone: "neutral",
      });
      return;
    }

    pushToast({
      eventKey: `external-import.import.failed.${noticeKey}`,
      taskId: importState.taskId ?? undefined,
      title: "批量导入失败",
      message: getExternalImportSelectionErrorMessage(importState.errorCode),
      tone: "danger",
    });
  }, [importState, pushToast]);

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
            getExternalImportSelectionErrorMessage("selection_candidate_invalid"),
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
        setSelectionError(getExternalImportSelectionErrorMessage(code));
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
      setSelectionError(getExternalImportSelectionErrorMessage(code));
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
        getExternalImportSelectionErrorMessage("selection_expired"),
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
      isImportActiveState(importStateRef.current) ||
      previewStateRef.current.status !== "ready" ||
      previewStateRef.current.loadingMore
    ) {
      return;
    }

    setTrackedPendingAction("start");
    setSelectionError(null);
    startPendingRef.current = true;
    pendingProgressEventsRef.current.clear();
    setTrackedImportState({ status: "starting" });
    try {
      const launch = await startExternalImportBatch({
        batchId: currentBatchId,
        selectionId: currentSelection.selectionId,
        expectedRevision: currentSelection.revision,
      });
      if (!isExternalImportBatchStartedDto(launch, currentBatchId)) {
        throw { code: "external_import_task_unavailable" };
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

      taskIdRef.current = launch.task.taskId;
      setTrackedSelection({ ...currentSelection, status: "sealed" });
      setTrackedImportState({
        status: "running",
        taskId: launch.task.taskId,
        phase: "external_import.import.queued",
        current: null,
        total: null,
      });
      const pendingEvent = pendingProgressEventsRef.current.get(launch.task.taskId);
      if (pendingEvent) {
        applyProgressEvent(pendingEvent);
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
      setTrackedImportState({
        status: "failed",
        taskId: null,
        phase: "external_import.import.start.failed",
        errorCode: code,
      });
      setSelectionError(getExternalImportSelectionErrorMessage(code));
    } finally {
      startPendingRef.current = false;
      pendingProgressEventsRef.current.clear();
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
    applyProgressEvent,
    isCurrentSelectionWorkflow,
    listenerStatus,
    setTrackedImportState,
    setTrackedPendingAction,
    setTrackedSelection,
  ]);

  const cancelImport = useCallback(async () => {
    const current = importStateRef.current;
    if (current.status !== "running" || cancelPendingRef.current) {
      return;
    }

    const generation = workflowGenerationRef.current;
    cancelPendingRef.current = true;
    setCancelPending(true);
    try {
      const cancelledTask = await cancelExternalImportTask({ taskId: current.taskId });
      if (
        cancelledTask.taskId !== current.taskId ||
        cancelledTask.kind !== "mod_import"
      ) {
        throw { code: "external_import_task_unavailable" };
      }
    } catch (error) {
      if (workflowGenerationRef.current !== generation) {
        return;
      }
      pushToast({
        eventKey: `external-import.import.cancel-failed.${current.taskId}`,
        taskId: current.taskId,
        title: "无法取消批量导入",
        message: getExternalImportSelectionErrorMessage(
          errorCodeFrom(error, "external_import_task_unavailable"),
        ),
        tone: "warning",
      });
    } finally {
      if (workflowGenerationRef.current === generation) {
        cancelPendingRef.current = false;
        setCancelPending(false);
      }
    }
  }, [pushToast]);

  const loadMore = useCallback(async () => {
    const currentBatchId = batchIdRef.current;
    const currentSelection = selectionRef.current;
    const currentPreview = previewStateRef.current;
    if (
      currentBatchId === null ||
      currentSelection === null ||
      currentSelection.status !== "editing" ||
      pendingActionRef.current !== null ||
      isImportActiveState(importStateRef.current) ||
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

      const incoming = page.candidates.map(toExternalImportPreviewCandidateViewModel);
      const candidates = appendExternalImportPreviewCandidates(
        currentPreview.candidates,
        page.candidates,
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
        ),
      });
    }
  }, [
    reconcileDecisionDrafts,
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

  const retryListener = useCallback(() => {
    if (listenerStatus !== "failed" || isImportActiveState(importStateRef.current)) {
      return;
    }
    setListenerStatus("loading");
    setListenerAttempt((attempt) => attempt + 1);
  }, [listenerStatus]);

  const runLoadMore = useCallback(() => {
    void loadMore();
  }, [loadMore]);

  const runSelectAll = useCallback(() => {
    void selectAll();
  }, [selectAll]);

  const runStartImport = useCallback(() => {
    void startImport();
  }, [startImport]);

  const runCancelImport = useCallback(() => {
    void cancelImport();
  }, [cancelImport]);

  const selectionEditable =
    selection !== null &&
    selection.status === "editing" &&
    !isExternalImportSelectionExpired(selection, Date.now()) &&
    !isImportActiveState(importState) &&
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
    importActive: isImportActiveState(importState),
    loadMore: runLoadMore,
    retryPreview,
    retryCategories,
    retryListener,
    setCandidateDecision,
    setCandidateSelected,
    selectAll: runSelectAll,
    startImport: runStartImport,
    cancelImport: runCancelImport,
  };
}
