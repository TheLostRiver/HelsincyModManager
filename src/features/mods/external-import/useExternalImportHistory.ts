import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import { useCallback, useRef, useState } from "react";
import {
  getExternalImportBatchResult,
  listExternalImportBatches,
} from "./externalImportApi";
import {
  appendExternalImportHistoryRows,
  getExternalImportHistoryErrorMessage,
  isExternalImportHistoryPage,
  toExternalImportHistoryRow,
  type ExternalImportHistoryRowViewModel,
} from "./externalImportHistoryModel";
import {
  appendExternalImportResults,
  isExternalImportHistoryBatchResultPage,
  summarizeExternalImportResults,
  toExternalImportResultViewModel,
  type ExternalImportResultSummary,
  type ExternalImportResultViewModel,
} from "./externalImportResultModel";

export type ExternalImportHistoryListState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "failed"; message: string }
  | { status: "empty" }
  | {
      status: "ready";
      rows: ExternalImportHistoryRowViewModel[];
      totalCount: number;
      nextCursor: string | null;
      loadingMore: boolean;
      loadMoreError: string | null;
    };

export type ExternalImportHistoryDetailState =
  | { status: "idle" }
  | { status: "loading"; batchId: string }
  | { status: "failed"; batchId: string; message: string }
  | {
      status: "ready";
      batchId: string;
      results: ExternalImportResultViewModel[];
      summary: ExternalImportResultSummary;
      totalCount: number;
      nextCursor: string | null;
      loadingMore: boolean;
      loadMoreError: string | null;
    };

export type ExternalImportHistoryWorkflow = {
  listState: ExternalImportHistoryListState;
  detailState: ExternalImportHistoryDetailState;
  ensureLoaded: () => void;
  refresh: () => void;
  loadMore: () => void;
  toggleDetails: (batchId: string) => void;
  reloadDetails: () => void;
  loadMoreDetails: () => void;
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

// 纯查询工作流:不持有任何任务事件 listener,数据源只有两个只读 command。
export function useExternalImportHistory(): ExternalImportHistoryWorkflow {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const [listState, setListState] = useState<ExternalImportHistoryListState>({
    status: "idle",
  });
  const [detailState, setDetailState] = useState<ExternalImportHistoryDetailState>({
    status: "idle",
  });
  const listStateRef = useRef<ExternalImportHistoryListState>(listState);
  const detailStateRef = useRef<ExternalImportHistoryDetailState>(detailState);
  const listRequestRef = useRef(0);
  const detailRequestRef = useRef(0);
  const extCopyRef = useRef(extCopy);
  const localeRef = useRef(locale);
  extCopyRef.current = extCopy;
  localeRef.current = locale;

  const setTrackedListState = useCallback((next: ExternalImportHistoryListState) => {
    listStateRef.current = next;
    setListState(next);
  }, []);

  const setTrackedDetailState = useCallback((next: ExternalImportHistoryDetailState) => {
    detailStateRef.current = next;
    setDetailState(next);
  }, []);

  const refresh = useCallback(() => {
    const requestId = listRequestRef.current + 1;
    listRequestRef.current = requestId;
    detailRequestRef.current += 1;
    setTrackedDetailState({ status: "idle" });
    setTrackedListState({ status: "loading" });
    void (async () => {
      try {
        const page = await listExternalImportBatches();
        if (!isExternalImportHistoryPage(page)) {
          throw { code: "external_import_history_invalid" };
        }
        if (listRequestRef.current !== requestId) {
          return;
        }
        if (page.batches.length === 0) {
          setTrackedListState({ status: "empty" });
          return;
        }
        const now = Date.now();
        setTrackedListState({
          status: "ready",
          rows: page.batches.map((entry) =>
            toExternalImportHistoryRow(entry, extCopyRef.current.history, localeRef.current, now),
          ),
          totalCount: page.totalCount,
          nextCursor: page.nextCursor,
          loadingMore: false,
          loadMoreError: null,
        });
      } catch (error) {
        if (listRequestRef.current !== requestId) {
          return;
        }
        setTrackedListState({
          status: "failed",
          message: getExternalImportHistoryErrorMessage(
            errorCodeFrom(error, "external_import_history_invalid"),
            extCopyRef.current.history,
          ),
        });
      }
    })();
  }, [setTrackedDetailState, setTrackedListState]);

  const ensureLoaded = useCallback(() => {
    if (listStateRef.current.status === "idle") {
      refresh();
    }
  }, [refresh]);

  const loadMore = useCallback(() => {
    const current = listStateRef.current;
    if (current.status !== "ready" || current.nextCursor === null || current.loadingMore) {
      return;
    }
    const requestId = listRequestRef.current + 1;
    listRequestRef.current = requestId;
    setTrackedListState({ ...current, loadingMore: true, loadMoreError: null });
    void (async () => {
      try {
        const page = await listExternalImportBatches({ cursor: current.nextCursor });
        if (!isExternalImportHistoryPage(page)) {
          throw { code: "external_import_history_invalid" };
        }
        if (listRequestRef.current !== requestId) {
          return;
        }
        setTrackedListState({
          status: "ready",
          rows: appendExternalImportHistoryRows(
            current.rows,
            page.batches,
            extCopyRef.current.history,
            localeRef.current,
            Date.now(),
          ),
          totalCount: page.totalCount,
          nextCursor: page.nextCursor,
          loadingMore: false,
          loadMoreError: null,
        });
      } catch (error) {
        if (listRequestRef.current !== requestId) {
          return;
        }
        setTrackedListState({
          ...current,
          loadingMore: false,
          loadMoreError: getExternalImportHistoryErrorMessage(
            errorCodeFrom(error, "external_import_history_invalid"),
            extCopyRef.current.history,
          ),
        });
      }
    })();
  }, [setTrackedListState]);

  const loadDetailFirstPage = useCallback(
    (batchId: string) => {
      const requestId = detailRequestRef.current + 1;
      detailRequestRef.current = requestId;
      setTrackedDetailState({ status: "loading", batchId });
      void (async () => {
        try {
          const page = await getExternalImportBatchResult({ batchId, cursor: null });
          if (!isExternalImportHistoryBatchResultPage(page, batchId)) {
            throw { code: "external_import_result_invalid" };
          }
          if (detailRequestRef.current !== requestId) {
            return;
          }
          const results = page.results.map((item) =>
            toExternalImportResultViewModel(item, extCopyRef.current.result),
          );
          setTrackedDetailState({
            status: "ready",
            batchId,
            results,
            summary: summarizeExternalImportResults(results),
            totalCount: page.totalCount,
            nextCursor: page.nextCursor,
            loadingMore: false,
            loadMoreError: null,
          });
        } catch (error) {
          if (detailRequestRef.current !== requestId) {
            return;
          }
          setTrackedDetailState({
            status: "failed",
            batchId,
            message: getExternalImportHistoryErrorMessage(
              errorCodeFrom(error, "external_import_result_invalid"),
              extCopyRef.current.history,
            ),
          });
        }
      })();
    },
    [setTrackedDetailState],
  );

  const toggleDetails = useCallback(
    (batchId: string) => {
      const current = detailStateRef.current;
      if (current.status !== "idle" && current.batchId === batchId) {
        detailRequestRef.current += 1;
        setTrackedDetailState({ status: "idle" });
        return;
      }
      loadDetailFirstPage(batchId);
    },
    [loadDetailFirstPage, setTrackedDetailState],
  );

  const reloadDetails = useCallback(() => {
    const current = detailStateRef.current;
    if (current.status === "failed" || current.status === "ready") {
      loadDetailFirstPage(current.batchId);
    }
  }, [loadDetailFirstPage]);

  const loadMoreDetails = useCallback(() => {
    const current = detailStateRef.current;
    if (current.status !== "ready" || current.nextCursor === null || current.loadingMore) {
      return;
    }
    const requestId = detailRequestRef.current + 1;
    detailRequestRef.current = requestId;
    setTrackedDetailState({ ...current, loadingMore: true, loadMoreError: null });
    void (async () => {
      try {
        const page = await getExternalImportBatchResult({
          batchId: current.batchId,
          cursor: current.nextCursor,
        });
        if (!isExternalImportHistoryBatchResultPage(page, current.batchId)) {
          throw { code: "external_import_result_invalid" };
        }
        if (detailRequestRef.current !== requestId) {
          return;
        }
        const results = appendExternalImportResults(
          current.results,
          page.results,
          extCopyRef.current.result,
        );
        setTrackedDetailState({
          status: "ready",
          batchId: current.batchId,
          results,
          summary: summarizeExternalImportResults(results),
          totalCount: page.totalCount,
          nextCursor: page.nextCursor,
          loadingMore: false,
          loadMoreError: null,
        });
      } catch (error) {
        if (detailRequestRef.current !== requestId) {
          return;
        }
        setTrackedDetailState({
          ...current,
          loadingMore: false,
          loadMoreError: getExternalImportHistoryErrorMessage(
            errorCodeFrom(error, "external_import_result_invalid"),
            extCopyRef.current.history,
          ),
        });
      }
    })();
  }, [setTrackedDetailState]);

  return {
    listState,
    detailState,
    ensureLoaded,
    refresh,
    loadMore,
    toggleDetails,
    reloadDetails,
    loadMoreDetails,
  };
}
