import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { useFeedback } from "../../../shared/feedback";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../modImportTypes";
import { cancelExternalImportTask } from "./externalImportApi";
import {
  getExternalImportPhaseLabel,
  isExternalImportTaskTerminal,
  nextExternalImportTaskStateFromProgress,
  type ExternalImportTaskState,
} from "./externalImportProgressState";
import {
  getExternalImportSelectionErrorMessage,
  isExternalImportBatchStartedDto,
} from "./externalImportSelectionModel";

export type ExternalImportListenerStatus = "loading" | "ready" | "failed";

export type ExternalImportLaunchResult =
  | { status: "started"; taskId: string }
  | { status: "failed"; errorCode: string }
  | { status: "stale" }
  | { status: "ignored" };

export type ExternalImportTaskProgressWorkflow = {
  importState: ExternalImportTaskState;
  listenerStatus: ExternalImportListenerStatus;
  cancelPending: boolean;
  importActive: boolean;
  isImportActive: () => boolean;
  launchImport: (
    startTask: () => Promise<unknown>,
  ) => Promise<ExternalImportLaunchResult>;
  retryListener: () => void;
  cancelImport: () => void;
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

function isImportActiveState(state: ExternalImportTaskState) {
  return (
    state.status === "starting" ||
    state.status === "running" ||
    state.status === "cancelling"
  );
}

export function useExternalImportTaskProgress(
  batchId: string | null,
): ExternalImportTaskProgressWorkflow {
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();
  const [importState, setImportState] = useState<ExternalImportTaskState>({
    status: "idle",
  });
  const importStateRef = useRef<ExternalImportTaskState>(importState);
  const [listenerStatus, setListenerStatus] =
    useState<ExternalImportListenerStatus>("loading");
  const listenerStatusRef = useRef<ExternalImportListenerStatus>(listenerStatus);
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [cancelPending, setCancelPending] = useState(false);
  const cancelPendingRef = useRef(false);
  const batchIdRef = useRef<string | null>(batchId);
  const generationRef = useRef(0);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const displayedTaskNoticeIdRef = useRef<string | null>(null);
  const terminalNoticeKeysRef = useRef(new Set<string>());
  batchIdRef.current = batchId;
  listenerStatusRef.current = listenerStatus;

  const setTrackedImportState = useCallback((next: ExternalImportTaskState) => {
    importStateRef.current = next;
    setImportState(next);
  }, []);

  const isImportActive = useCallback(
    () => isImportActiveState(importStateRef.current),
    [],
  );

  useEffect(() => {
    generationRef.current += 1;
    taskIdRef.current = null;
    startPendingRef.current = false;
    pendingProgressEventsRef.current.clear();
    terminalNoticeKeysRef.current.clear();
    cancelPendingRef.current = false;
    setCancelPending(false);
    setTrackedImportState({ status: "idle" });
  }, [batchId, setTrackedImportState]);

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
        message: "正在读取服务端确认的结果明细。",
        tone: "success",
      });
      return;
    }
    if (importState.status === "cancelled") {
      pushToast({
        eventKey: `external-import.import.cancelled.${noticeKey}`,
        taskId: importState.taskId,
        title: "批量导入已取消",
        message: "正在读取已保留的权威结果。",
        tone: "neutral",
      });
      return;
    }
    pushToast({
      eventKey: `external-import.import.failed.${noticeKey}`,
      taskId: importState.taskId ?? undefined,
      title: "批量导入未完成",
      message: getExternalImportSelectionErrorMessage(importState.errorCode),
      tone: "danger",
    });
  }, [importState, pushToast]);

  const launchImport = useCallback(
    async (
      startTask: () => Promise<unknown>,
    ): Promise<ExternalImportLaunchResult> => {
      const expectedBatchId = batchIdRef.current;
      const generation = generationRef.current;
      if (
        expectedBatchId === null ||
        listenerStatusRef.current !== "ready" ||
        startPendingRef.current ||
        isImportActiveState(importStateRef.current)
      ) {
        return { status: "ignored" };
      }

      startPendingRef.current = true;
      pendingProgressEventsRef.current.clear();
      setTrackedImportState({ status: "starting" });
      try {
        const launch = await startTask();
        if (!isExternalImportBatchStartedDto(launch, expectedBatchId)) {
          throw { code: "external_import_task_unavailable" };
        }
        if (
          generationRef.current !== generation ||
          batchIdRef.current !== expectedBatchId
        ) {
          return { status: "stale" };
        }

        taskIdRef.current = launch.task.taskId;
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
        return { status: "started", taskId: launch.task.taskId };
      } catch (error) {
        if (
          generationRef.current !== generation ||
          batchIdRef.current !== expectedBatchId
        ) {
          return { status: "stale" };
        }
        const errorCode = errorCodeFrom(
          error,
          "external_import_task_unavailable",
        );
        setTrackedImportState({
          status: "failed",
          taskId: null,
          phase: "external_import.import.start.failed",
          errorCode,
        });
        return { status: "failed", errorCode };
      } finally {
        if (generationRef.current === generation) {
          startPendingRef.current = false;
          pendingProgressEventsRef.current.clear();
        }
      }
    },
    [applyProgressEvent, setTrackedImportState],
  );

  const cancelImportRequest = useCallback(async () => {
    const current = importStateRef.current;
    if (current.status !== "running" || cancelPendingRef.current) {
      return;
    }

    const generation = generationRef.current;
    cancelPendingRef.current = true;
    setCancelPending(true);
    try {
      const cancelledTask = await cancelExternalImportTask({
        taskId: current.taskId,
      });
      if (
        cancelledTask.taskId !== current.taskId ||
        cancelledTask.kind !== "mod_import"
      ) {
        throw { code: "external_import_task_unavailable" };
      }
    } catch (error) {
      if (generationRef.current !== generation) {
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
      if (generationRef.current === generation) {
        cancelPendingRef.current = false;
        setCancelPending(false);
      }
    }
  }, [pushToast]);

  const cancelImport = useCallback(() => {
    void cancelImportRequest();
  }, [cancelImportRequest]);

  const retryListener = useCallback(() => {
    if (
      listenerStatusRef.current !== "failed" ||
      isImportActiveState(importStateRef.current)
    ) {
      return;
    }
    setListenerStatus("loading");
    setListenerAttempt((attempt) => attempt + 1);
  }, []);

  return {
    importState,
    listenerStatus,
    cancelPending,
    importActive: isImportActiveState(importState),
    isImportActive,
    launchImport,
    retryListener,
    cancelImport,
  };
}
