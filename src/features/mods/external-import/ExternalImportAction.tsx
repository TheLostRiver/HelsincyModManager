import { listen } from "@tauri-apps/api/event";
import { CircleAlert, FolderInput, LoaderCircle, RefreshCcw, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog, useFeedback } from "../../../shared/feedback";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../modImportTypes";
import {
  cancelExternalImportScan,
  selectExternalImportSource,
  startExternalImportScan,
} from "./externalImportApi";
import { ExternalImportSelectionPanel } from "./ExternalImportSelectionPanel";
import {
  getExternalImportScanErrorMessage,
  getExternalImportScanPhaseLabel,
  isExternalImportScanTaskTerminal,
  nextExternalImportScanTaskStateFromProgress,
  type ExternalImportScanTaskState,
} from "./externalImportScanState";
import {
  isExternalImportOpaqueId,
  isExternalImportSourceDto,
  type ExternalImportSourceDto,
} from "./externalImportTypes.ts";
import { useExternalImportSelectionWorkflow } from "./useExternalImportSelectionWorkflow";
import "./ExternalImportAction.css";

type ListenerStatus = "loading" | "ready" | "failed";

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

function hasExpectedScanLaunch(value: {
  task: { taskId: string; kind: string; status: string };
  batchId: string;
}) {
  return (
    value.task.kind === "mod_import" &&
    value.task.status === "queued" &&
    isExternalImportOpaqueId(value.task.taskId) &&
    isExternalImportOpaqueId(value.batchId)
  );
}

function isScanActive(state: ExternalImportScanTaskState) {
  return state.status === "starting" || state.status === "running";
}

type ExternalImportActionProps = {
  onImported: () => Promise<void> | void;
};

export function ExternalImportAction({ onImported }: ExternalImportActionProps) {
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();
  const chooseSourceButtonRef = useRef<HTMLButtonElement | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [listenerStatus, setListenerStatus] = useState<ListenerStatus>("loading");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [sourcePickerActive, setSourcePickerActive] = useState(false);
  const [cancelPending, setCancelPending] = useState(false);
  const [source, setSource] = useState<ExternalImportSourceDto | null>(null);
  const [batchId, setBatchId] = useState<string | null>(null);
  const [scanState, setScanState] = useState<ExternalImportScanTaskState>({ status: "idle" });
  const scanStateRef = useRef<ExternalImportScanTaskState>(scanState);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const displayedTaskNoticeIdRef = useRef<string | null>(null);
  const terminalNoticeKeysRef = useRef(new Set<string>());
  const selectionWorkflow = useExternalImportSelectionWorkflow(
    scanState.status === "completed" ? batchId : null,
    onImported,
  );
  const selectionWorkflowBusy =
    selectionWorkflow.pendingAction !== null ||
    selectionWorkflow.importActive ||
    selectionWorkflow.previewState.status === "loading" ||
    (
      selectionWorkflow.previewState.status === "ready" &&
      selectionWorkflow.previewState.loadingMore
    );

  const setTrackedScanState = useCallback((next: ExternalImportScanTaskState) => {
    scanStateRef.current = next;
    setScanState(next);
  }, []);

  const resetWorkflow = useCallback(() => {
    taskIdRef.current = null;
    pendingProgressEventsRef.current.clear();
    setSource(null);
    setBatchId(null);
    setTrackedScanState({ status: "idle" });
  }, [setTrackedScanState]);

  const applyProgressEvent = useCallback(
    (event: TaskProgressEventDto) => {
      const current = scanStateRef.current;
      const next = nextExternalImportScanTaskStateFromProgress(current, event);
      if (next === current) {
        return;
      }

      if (isExternalImportScanTaskTerminal(next)) {
        taskIdRef.current = null;
      }
      setTrackedScanState(next);
    },
    [setTrackedScanState],
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
          taskIdRef.current = null;
          setListenerStatus("failed");
        }
      });

    return () => {
      disposed = true;
      unlistenTaskProgress?.();
    };
  }, [applyProgressEvent, listenerAttempt, setTrackedScanState]);

  useEffect(() => {
    const previousTaskId = displayedTaskNoticeIdRef.current;
    if (scanState.status === "running") {
      if (previousTaskId && previousTaskId !== scanState.taskId) {
        dismissTaskNotice(previousTaskId);
      }
      displayedTaskNoticeIdRef.current = scanState.taskId;
      showTaskNotice({
        taskId: scanState.taskId,
        title: "正在扫描第三方来源",
        message: getExternalImportScanPhaseLabel(scanState.phase),
        tone: "progress",
      });
      return;
    }

    if (previousTaskId) {
      dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = null;
    }
  }, [dismissTaskNotice, scanState, showTaskNotice]);

  useEffect(() => () => {
    const taskId = displayedTaskNoticeIdRef.current;
    if (taskId) {
      dismissTaskNotice(taskId);
    }
  }, [dismissTaskNotice]);

  useEffect(() => {
    if (scanState.status !== "failed" && scanState.status !== "cancelled") {
      return;
    }

    const noticeKey = `${scanState.status}.${scanState.taskId ?? scanState.phase}`;
    if (terminalNoticeKeysRef.current.has(noticeKey)) {
      return;
    }
    terminalNoticeKeysRef.current.add(noticeKey);

    if (scanState.status === "failed") {
      pushToast({
        eventKey: `external-import.failed.${noticeKey}`,
        taskId: scanState.taskId ?? undefined,
        title: "第三方来源扫描失败",
        message: getExternalImportScanErrorMessage(scanState.errorCode),
        tone: "danger",
      });
      return;
    }

    pushToast({
      eventKey: `external-import.cancelled.${noticeKey}`,
      taskId: scanState.taskId,
      title: "第三方来源扫描已取消",
      message: "未创建可导入选择。",
      tone: "neutral",
    });
  }, [pushToast, scanState]);

  const launchScan = useCallback(
    async (selectedSource: ExternalImportSourceDto) => {
      startPendingRef.current = true;
      pendingProgressEventsRef.current.clear();
      setBatchId(null);
      setTrackedScanState({ status: "starting" });

      try {
        const launch = await startExternalImportScan({ sourceId: selectedSource.sourceId });
        if (!hasExpectedScanLaunch(launch)) {
          setTrackedScanState({
            status: "failed",
            taskId: null,
            phase: "external_import.scan.start.invalid",
            errorCode: "external_import_task_unavailable",
          });
          return;
        }

        setBatchId(launch.batchId);
        taskIdRef.current = launch.task.taskId;
        const startedState: ExternalImportScanTaskState = {
          status: "running",
          taskId: launch.task.taskId,
          phase: "external_import.scan.queued",
        };
        setTrackedScanState(startedState);

        const pendingEvent = pendingProgressEventsRef.current.get(launch.task.taskId);
        if (pendingEvent) {
          applyProgressEvent(pendingEvent);
        }
      } catch (error) {
        setTrackedScanState({
          status: "failed",
          taskId: null,
          phase: "external_import.scan.start.failed",
          errorCode: errorCodeFrom(error, "external_import_scan_failed"),
        });
      } finally {
        startPendingRef.current = false;
        pendingProgressEventsRef.current.clear();
      }
    },
    [applyProgressEvent, setTrackedScanState],
  );

  const chooseSource = useCallback(async () => {
    if (
      listenerStatus !== "ready" ||
      sourcePickerActive ||
      isScanActive(scanStateRef.current) ||
      selectionWorkflowBusy
    ) {
      return;
    }

    resetWorkflow();
    setDialogOpen(true);
    setSourcePickerActive(true);
    try {
      const selectedSource = await selectExternalImportSource();
      if (selectedSource === null) {
        return;
      }
      if (!isExternalImportSourceDto(selectedSource)) {
        setTrackedScanState({
          status: "failed",
          taskId: null,
          phase: "external_import.source_picker.invalid",
          errorCode: "external_import_source_unavailable",
        });
        return;
      }
      setSource(selectedSource);
      await launchScan(selectedSource);
    } catch (error) {
      setTrackedScanState({
        status: "failed",
        taskId: null,
        phase: "external_import.source_picker.failed",
        errorCode: errorCodeFrom(error, "external_import_source_picker_unavailable"),
      });
    } finally {
      setSourcePickerActive(false);
    }
  }, [
    launchScan,
    listenerStatus,
    resetWorkflow,
    selectionWorkflowBusy,
    setTrackedScanState,
    sourcePickerActive,
  ]);

  const requestCancel = useCallback(async () => {
    if (scanState.status !== "running" || cancelPending) {
      return;
    }

    setCancelPending(true);
    try {
      const cancelledTask = await cancelExternalImportScan({ taskId: scanState.taskId });
      if (cancelledTask.taskId !== scanState.taskId || cancelledTask.kind !== "mod_import") {
        throw { code: "external_import_task_unavailable" };
      }
    } catch (error) {
      pushToast({
        eventKey: `external-import.cancel-failed.${scanState.taskId}`,
        taskId: scanState.taskId,
        title: "无法取消扫描",
        message: getExternalImportScanErrorMessage(errorCodeFrom(error, "external_import_task_unavailable")),
        tone: "warning",
      });
    } finally {
      setCancelPending(false);
    }
  }, [cancelPending, pushToast, scanState]);

  function retryListener() {
    if (listenerStatus !== "failed" || isScanActive(scanStateRef.current)) {
      return;
    }
    resetWorkflow();
    setListenerStatus("loading");
    setListenerAttempt((attempt) => attempt + 1);
  }

  function openDialog() {
    setDialogOpen(true);
    if (scanStateRef.current.status === "idle" && source === null && listenerStatus === "ready") {
      void chooseSource();
    }
  }

  const scanStatusText =
    sourcePickerActive
      ? "正在选择来源"
      : scanState.status === "starting"
        ? "正在创建扫描任务"
        : scanState.status === "running"
          ? getExternalImportScanPhaseLabel(scanState.phase)
          : null;
  const sourceButtonDisabled =
    listenerStatus !== "ready" ||
    sourcePickerActive ||
    isScanActive(scanState) ||
    selectionWorkflowBusy;

  return (
    <>
      <button
        type="button"
        className="compact-action is-neutral external-import-action__trigger"
        disabled={listenerStatus === "loading"}
        aria-label="迁移第三方 Mod"
        onClick={openDialog}
      >
        <span className="compact-action__left">
          <FolderInput size={14} strokeWidth={2.4} aria-hidden="true" />
          <span className="compact-action__label">迁移第三方 Mod</span>
        </span>
      </button>

      <Dialog
        open={dialogOpen}
        title="第三方 Mod 迁移"
        description={source?.displayLabel ?? "只读扫描与候选预览"}
        icon={<FolderInput size={20} />}
        busy={
          sourcePickerActive ||
          scanState.status === "starting" ||
          selectionWorkflow.importState.status === "starting"
        }
        initialFocusRef={chooseSourceButtonRef}
        onClose={() => setDialogOpen(false)}
        footer={
          <>
            {scanState.status === "running" ? (
              <button
                type="button"
                className="external-import__button is-danger"
                disabled={cancelPending}
                onClick={() => void requestCancel()}
              >
                {cancelPending ? <LoaderCircle className="external-import__spinner" size={15} /> : <XCircle size={15} />}
                {cancelPending ? "正在取消" : "取消扫描"}
              </button>
            ) : null}
            {listenerStatus === "failed" ? (
              <button type="button" className="external-import__button is-secondary" onClick={retryListener}>
                <RefreshCcw size={15} />
                重试状态监听
              </button>
            ) : null}
            <button
              ref={chooseSourceButtonRef}
              type="button"
              className="external-import__button is-primary"
              disabled={sourceButtonDisabled}
              onClick={() => void chooseSource()}
            >
              {sourcePickerActive ? <LoaderCircle className="external-import__spinner" size={15} /> : <FolderInput size={15} />}
              选择来源
            </button>
          </>
        }
      >
        <div className="external-import">
          <div className="external-import__source-row">
            <span className="external-import__eyebrow">来源</span>
            <strong>{source?.displayLabel ?? "尚未选择"}</strong>
          </div>

          {listenerStatus === "loading" ? (
            <div className="external-import__state" role="status" aria-live="polite">
              <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
              <span>正在连接扫描状态</span>
            </div>
          ) : null}

          {listenerStatus === "failed" ? (
            <div className="external-import__state is-error" role="alert">
              <CircleAlert size={18} aria-hidden="true" />
              <span>{getExternalImportScanErrorMessage("external_import_listener_unavailable")}</span>
            </div>
          ) : null}

          {scanStatusText ? (
            <div className="external-import__state" role="status" aria-live="polite">
              <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
              <span>{scanStatusText}</span>
            </div>
          ) : null}

          {scanState.status === "failed" ? (
            <div className="external-import__state is-error" role="alert">
              <CircleAlert size={18} aria-hidden="true" />
              <span>{getExternalImportScanErrorMessage(scanState.errorCode)}</span>
            </div>
          ) : null}

          {scanState.status === "cancelled" ? (
            <div className="external-import__state is-muted" role="status" aria-live="polite">
              <XCircle size={18} aria-hidden="true" />
              <span>扫描已取消</span>
            </div>
          ) : null}

          {scanState.status === "completed" && batchId ? (
            <ExternalImportSelectionPanel workflow={selectionWorkflow} />
          ) : null}
        </div>
      </Dialog>
    </>
  );
}
