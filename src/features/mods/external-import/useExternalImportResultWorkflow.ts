import { useCallback, useEffect, useRef, useState } from "react";
import { useFeedback } from "../../../shared/feedback";
import {
  getExternalImportBatchResult,
  retryExternalImportBatch,
} from "./externalImportApi";
import {
  appendExternalImportResults,
  getExternalImportResultErrorMessage,
  isExternalImportBatchResultPageForBatch,
  isExternalImportResultCoverageValid,
  summarizeExternalImportResults,
  toExternalImportResultViewModel,
  type ExternalImportResultSummary,
  type ExternalImportResultViewModel,
} from "./externalImportResultModel";
import {
  isExternalImportTaskTerminal,
  type ExternalImportTaskState,
} from "./externalImportProgressState";
import type { ExternalImportBatchImportStatus } from "./externalImportTypes";
import type { ExternalImportLaunchResult } from "./useExternalImportTaskProgress";

export type ExternalImportResultState =
  | { status: "idle" }
  | { status: "loading"; taskId: string }
  | { status: "failed"; taskId: string; message: string }
  | {
      status: "empty";
      taskId: string;
      batchStatus: ExternalImportBatchImportStatus;
      totalCount: number;
    }
  | {
      status: "ready";
      taskId: string;
      batchStatus: ExternalImportBatchImportStatus;
      results: ExternalImportResultViewModel[];
      totalCount: number;
      nextCursor: string | null;
      loadingMore: boolean;
      loadMoreError: string | null;
    };

export type ExternalImportResultWorkflow = {
  state: ExternalImportResultState;
  summary: ExternalImportResultSummary;
  retryPending: boolean;
  retryAvailable: boolean;
  resultStale: boolean;
  actionError: string | null;
  loadMore: () => void;
  retryResultQuery: () => void;
  retryResults: () => void;
};

type UseExternalImportResultWorkflowInput = {
  batchId: string | null;
  selectionId: string | null;
  importState: ExternalImportTaskState;
  importActive: boolean;
  progressReady: boolean;
  launchImport: (
    startTask: () => Promise<unknown>,
  ) => Promise<ExternalImportLaunchResult>;
  onImported: () => Promise<void> | void;
};

const emptySummary: ExternalImportResultSummary = {
  imported: 0,
  alreadyImported: 0,
  skipped: 0,
  blocked: 0,
  failed: 0,
  cancelled: 0,
  retryable: 0,
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

function canRetryState(state: ExternalImportResultState) {
  return (
    state.status === "ready" &&
    state.batchStatus !== "completed" &&
    (
      state.results.some((result) => result.retryable) ||
      state.nextCursor !== null
    )
  );
}

export function useExternalImportResultWorkflow({
  batchId,
  selectionId,
  importState,
  importActive,
  progressReady,
  launchImport,
  onImported,
}: UseExternalImportResultWorkflowInput): ExternalImportResultWorkflow {
  const { pushToast } = useFeedback();
  const [state, setState] = useState<ExternalImportResultState>({
    status: "idle",
  });
  const stateRef = useRef<ExternalImportResultState>(state);
  const [observedBatchId, setObservedBatchId] = useState<string | null>(batchId);
  const batchChanged = observedBatchId !== batchId;
  const [retryPending, setRetryPending] = useState(false);
  const retryPendingRef = useRef(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const batchIdRef = useRef<string | null>(batchId);
  const selectionIdRef = useRef<string | null>(selectionId);
  const generationRef = useRef(0);
  const resultRequestRef = useRef(0);
  const terminalTaskIdRef = useRef<string | null>(null);
  const refreshedTaskIdsRef = useRef(new Set<string>());
  const onImportedRef = useRef(onImported);
  batchIdRef.current = batchId;
  selectionIdRef.current = selectionId;

  const setTrackedState = useCallback((next: ExternalImportResultState) => {
    stateRef.current = next;
    setState(next);
  }, []);

  useEffect(() => {
    onImportedRef.current = onImported;
  }, [onImported]);

  useEffect(() => {
    generationRef.current += 1;
    resultRequestRef.current += 1;
    terminalTaskIdRef.current = null;
    refreshedTaskIdsRef.current.clear();
    retryPendingRef.current = false;
    setRetryPending(false);
    setActionError(null);
    setObservedBatchId(batchId);
    setTrackedState({ status: "idle" });
  }, [batchId, setTrackedState]);

  const refreshLibraryOnce = useCallback(
    (taskId: string) => {
      if (refreshedTaskIdsRef.current.has(taskId)) {
        return;
      }
      refreshedTaskIdsRef.current.add(taskId);
      void Promise.resolve()
        .then(() => onImportedRef.current())
        .then(
          () =>
            pushToast({
              eventKey: `external-import.result.refreshed.${taskId}`,
              taskId,
              title: "批量导入结果已载入",
              message: "Mod 列表与服务端确认的结果明细已刷新。",
              tone: "success",
            }),
          () =>
            pushToast({
              eventKey: `external-import.result.refresh-failed.${taskId}`,
              taskId,
              title: "结果已载入，Mod 列表刷新失败",
              message: "导入事实不受影响，请稍后手动刷新 Mod 列表。",
              tone: "warning",
            }),
        );
    },
    [pushToast],
  );

  const loadFirstPage = useCallback(
    async (expectedBatchId: string, expectedTaskId: string) => {
      const requestId = resultRequestRef.current + 1;
      resultRequestRef.current = requestId;
      setActionError(null);
      setTrackedState({ status: "loading", taskId: expectedTaskId });
      try {
        const page = await getExternalImportBatchResult({
          batchId: expectedBatchId,
          cursor: null,
        });
        if (!isExternalImportBatchResultPageForBatch(page, expectedBatchId)) {
          throw { code: "external_import_result_invalid" };
        }
        if (
          resultRequestRef.current !== requestId ||
          batchIdRef.current !== expectedBatchId ||
          terminalTaskIdRef.current !== expectedTaskId
        ) {
          return false;
        }

        const results = page.results.map(toExternalImportResultViewModel);
        if (
          !isExternalImportResultCoverageValid(
            page.totalCount,
            page.nextCursor,
            results.length,
          )
        ) {
          throw { code: "external_import_result_invalid" };
        }
        setTrackedState(
          results.length === 0
            ? {
                status: "empty",
                taskId: expectedTaskId,
                batchStatus: page.batch.importStatus,
                totalCount: page.totalCount,
              }
            : {
                status: "ready",
                taskId: expectedTaskId,
                batchStatus: page.batch.importStatus,
                results,
                totalCount: page.totalCount,
                nextCursor: page.nextCursor,
                loadingMore: false,
                loadMoreError: null,
              },
        );
        refreshLibraryOnce(expectedTaskId);
        return true;
      } catch (error) {
        if (
          resultRequestRef.current !== requestId ||
          batchIdRef.current !== expectedBatchId ||
          terminalTaskIdRef.current !== expectedTaskId
        ) {
          return false;
        }
        setTrackedState({
          status: "failed",
          taskId: expectedTaskId,
          message: getExternalImportResultErrorMessage(
            errorCodeFrom(error, "external_import_result_invalid"),
          ),
        });
        return false;
      }
    },
    [refreshLibraryOnce, setTrackedState],
  );

  useEffect(() => {
    if (batchChanged) {
      terminalTaskIdRef.current = null;
      resultRequestRef.current += 1;
      return;
    }
    if (!isExternalImportTaskTerminal(importState) || importState.taskId === null) {
      terminalTaskIdRef.current = null;
      resultRequestRef.current += 1;
      return;
    }

    terminalTaskIdRef.current = importState.taskId;
    const expectedBatchId = batchIdRef.current;
    if (expectedBatchId !== null) {
      void loadFirstPage(expectedBatchId, importState.taskId);
    }
  }, [batchChanged, importState, loadFirstPage]);

  const loadMoreRequest = useCallback(async () => {
    const current = stateRef.current;
    const expectedBatchId = batchIdRef.current;
    if (
      current.status !== "ready" ||
      expectedBatchId === null ||
      current.nextCursor === null ||
      current.loadingMore ||
      importActive ||
      retryPendingRef.current
    ) {
      return;
    }

    const requestId = resultRequestRef.current + 1;
    resultRequestRef.current = requestId;
    setTrackedState({
      ...current,
      loadingMore: true,
      loadMoreError: null,
    });
    try {
      const page = await getExternalImportBatchResult({
        batchId: expectedBatchId,
        cursor: current.nextCursor,
      });
      if (
        !isExternalImportBatchResultPageForBatch(page, expectedBatchId) ||
        page.batch.importStatus !== current.batchStatus ||
        page.totalCount !== current.totalCount ||
        (
          page.nextCursor !== null &&
          Number(page.nextCursor) <= Number(current.nextCursor)
        )
      ) {
        throw { code: "external_import_result_invalid" };
      }
      if (
        resultRequestRef.current !== requestId ||
        batchIdRef.current !== expectedBatchId ||
        terminalTaskIdRef.current !== current.taskId
      ) {
        return;
      }

      const mergedResults = appendExternalImportResults(
        current.results,
        page.results,
      );
      if (
        !isExternalImportResultCoverageValid(
          page.totalCount,
          page.nextCursor,
          mergedResults.length,
        )
      ) {
        throw { code: "external_import_result_invalid" };
      }
      setTrackedState({
        ...current,
        results: mergedResults,
        nextCursor: page.nextCursor,
        loadingMore: false,
        loadMoreError: null,
      });
    } catch (error) {
      if (
        resultRequestRef.current !== requestId ||
        batchIdRef.current !== expectedBatchId ||
        terminalTaskIdRef.current !== current.taskId
      ) {
        return;
      }
      setTrackedState({
        ...current,
        loadingMore: false,
        loadMoreError: getExternalImportResultErrorMessage(
          errorCodeFrom(error, "external_import_result_invalid"),
        ),
      });
    }
  }, [importActive, setTrackedState]);

  const loadMore = useCallback(() => {
    void loadMoreRequest();
  }, [loadMoreRequest]);

  const retryResultQuery = useCallback(() => {
    const expectedBatchId = batchIdRef.current;
    const expectedTaskId = terminalTaskIdRef.current;
    if (expectedBatchId === null || expectedTaskId === null || importActive) {
      return;
    }
    void loadFirstPage(expectedBatchId, expectedTaskId);
  }, [importActive, loadFirstPage]);

  const retryResultsRequest = useCallback(async () => {
    const currentBatchId = batchIdRef.current;
    const currentSelectionId = selectionIdRef.current;
    const currentState = stateRef.current;
    const generation = generationRef.current;
    if (
      currentBatchId === null ||
      currentSelectionId === null ||
      !canRetryState(currentState) ||
      (currentState.status === "ready" && currentState.loadingMore) ||
      importActive ||
      !progressReady ||
      retryPendingRef.current
    ) {
      return;
    }

    retryPendingRef.current = true;
    setRetryPending(true);
    setActionError(null);
    try {
      const launchResult = await launchImport(() =>
        retryExternalImportBatch({
          batchId: currentBatchId,
          selectionId: currentSelectionId,
        }),
      );
      if (
        generationRef.current !== generation ||
        batchIdRef.current !== currentBatchId
      ) {
        return;
      }
      if (launchResult.status === "failed") {
        setActionError(
          getExternalImportResultErrorMessage(launchResult.errorCode),
        );
      }
    } finally {
      if (generationRef.current === generation) {
        retryPendingRef.current = false;
        setRetryPending(false);
      }
    }
  }, [importActive, launchImport, progressReady]);

  const retryResults = useCallback(() => {
    void retryResultsRequest();
  }, [retryResultsRequest]);

  const visibleState: ExternalImportResultState = batchChanged
    ? { status: "idle" }
    : state;
  const summary =
    visibleState.status === "ready"
      ? summarizeExternalImportResults(visibleState.results)
      : emptySummary;

  return {
    state: visibleState,
    summary,
    retryPending,
    retryAvailable: progressReady && canRetryState(visibleState),
    resultStale:
      importActive &&
      (visibleState.status === "ready" || visibleState.status === "empty"),
    actionError,
    loadMore,
    retryResultQuery,
    retryResults,
  };
}
