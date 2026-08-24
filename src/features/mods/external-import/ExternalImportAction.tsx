import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import { listen } from "@tauri-apps/api/event";
import {
  CircleAlert,
  FolderInput,
  History,
  LoaderCircle,
  RefreshCcw,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog, useFeedback } from "../../../shared/feedback";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../modImportTypes";
import {
  cancelExternalImportScan,
  selectExternalImportSource,
  startExternalImportScan,
} from "./externalImportApi";
import { ExternalImportHistoryPanel } from "./ExternalImportHistoryPanel";
import { ExternalImportSelectionPanel } from "./ExternalImportSelectionPanel";
import { useExternalImportHistory } from "./useExternalImportHistory";
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
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();
  const chooseSourceButtonRef = useRef<HTMLButtonElement | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [view, setView] = useState<"current" | "history">("current");
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
  const historyWorkflow = useExternalImportHistory();
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
        title: extCopy.action.scanningToastTitle,
        message: getExternalImportScanPhaseLabel(scanState.phase, extCopy.scan),
        tone: "progress",
      });
      return;
    }

    if (previousTaskId) {
      dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = null;
    }
  }, [dismissTaskNotice, extCopy, scanState, showTaskNotice]);

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
        title: extCopy.action.scanFailedToastTitle,
        message: getExternalImportScanErrorMessage(scanState.errorCode, extCopy.scan),
        tone: "danger",
      });
      return;
    }

    pushToast({
      eventKey: `external-import.cancelled.${noticeKey}`,
      taskId: scanState.taskId,
      title: extCopy.action.scanCancelledToastTitle,
      message: extCopy.action.scanCancelledToastMessage,
      tone: "neutral",
    });
  }, [extCopy, pushToast, scanState]);

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
        title: extCopy.action.cancelScanFailedTitle,
        message: getExternalImportScanErrorMessage(
            errorCodeFrom(error, "external_import_task_unavailable"),
            extCopy.scan,
          ),
        tone: "warning",
      });
    } finally {
      setCancelPending(false);
    }
  }, [cancelPending, extCopy, pushToast, scanState]);

  function retryListener() {
    if (listenerStatus !== "failed" || isScanActive(scanStateRef.current)) {
      return;
    }
    resetWorkflow();
    setListenerStatus("loading");
    setListenerAttempt((attempt) => attempt + 1);
  }

  function openDialog() {
    setView("current");
    setDialogOpen(true);
    if (scanStateRef.current.status === "idle" && source === null && listenerStatus === "ready") {
      void chooseSource();
    }
  }

  // 记录模式打开:纯查询视图,绝不拉起原生目录选择器。
  function openHistory() {
    setView("history");
    setDialogOpen(true);
    historyWorkflow.ensureLoaded();
  }

  function switchView(next: "current" | "history") {
    setView(next);
    if (next === "history") {
      historyWorkflow.ensureLoaded();
    }
  }

  const scanStatusText =
    sourcePickerActive
      ? extCopy.action.choosingSource
      : scanState.status === "starting"
        ? extCopy.action.creatingScanTask
        : scanState.status === "running"
          ? getExternalImportScanPhaseLabel(scanState.phase, extCopy.scan)
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
        aria-label={extCopy.action.trigger}
        onClick={openDialog}
      >
        <span className="compact-action__left">
          <FolderInput size={14} strokeWidth={2.4} aria-hidden="true" />
          <span className="compact-action__label">{extCopy.action.trigger}</span>
        </span>
      </button>
      <button
        type="button"
        className="compact-action is-neutral external-import-action__history-trigger"
        aria-label={extCopy.history.historyTriggerAria}
        title={extCopy.history.historyTriggerAria}
        onClick={openHistory}
      >
        <span className="compact-action__left">
          <History size={14} strokeWidth={2.4} aria-hidden="true" />
        </span>
      </button>

      <Dialog
        open={dialogOpen}
        title={extCopy.action.dialogTitle}
        description={
          view === "history"
            ? extCopy.history.tabHistory
            : source?.displayLabel ?? extCopy.action.dialogFallbackDescription
        }
        icon={<FolderInput size={20} />}
        busy={
          sourcePickerActive ||
          scanState.status === "starting" ||
          selectionWorkflow.importState.status === "starting"
        }
        initialFocusRef={chooseSourceButtonRef}
        onClose={() => setDialogOpen(false)}
        footer={
          view === "history" ? undefined : (
          <>
            {scanState.status === "running" ? (
              <button
                type="button"
                className="external-import__button is-danger"
                disabled={cancelPending}
                onClick={() => void requestCancel()}
              >
                {cancelPending ? <LoaderCircle className="external-import__spinner" size={15} /> : <XCircle size={15} />}
                {cancelPending ? extCopy.action.cancelling : extCopy.action.cancelScan}
              </button>
            ) : null}
            {listenerStatus === "failed" ? (
              <button type="button" className="external-import__button is-secondary" onClick={retryListener}>
                <RefreshCcw size={15} />
                {extCopy.action.retryListener}
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
              {extCopy.action.chooseSource}
            </button>
          </>
          )
        }
      >
        <div className="external-import">
          <div
            className="external-import__tabs"
            role="tablist"
            aria-label={extCopy.history.tablistAria}
          >
            <button
              type="button"
              role="tab"
              aria-selected={view === "current"}
              className={view === "current" ? "is-active" : undefined}
              disabled={sourcePickerActive}
              onClick={() => switchView("current")}
            >
              <FolderInput size={15} aria-hidden="true" />
              {extCopy.history.tabCurrent}
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={view === "history"}
              className={view === "history" ? "is-active" : undefined}
              disabled={sourcePickerActive}
              onClick={() => switchView("history")}
            >
              <History size={15} aria-hidden="true" />
              {extCopy.history.tabHistory}
            </button>
          </div>

          {view === "history" ? (
            <ExternalImportHistoryPanel workflow={historyWorkflow} />
          ) : (
          <>
          <div className="external-import__source-row">
            <span className="external-import__eyebrow">{extCopy.action.sourceEyebrow}</span>
            <strong>{source?.displayLabel ?? extCopy.action.sourceNotChosen}</strong>
          </div>

          {listenerStatus === "loading" ? (
            <div className="external-import__state" role="status" aria-live="polite">
              <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
              <span>{extCopy.action.connectingScanStatus}</span>
            </div>
          ) : null}

          {listenerStatus === "failed" ? (
            <div className="external-import__state is-error" role="alert">
              <CircleAlert size={18} aria-hidden="true" />
              <span>{getExternalImportScanErrorMessage("external_import_listener_unavailable", extCopy.scan)}</span>
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
              <span>{getExternalImportScanErrorMessage(scanState.errorCode, extCopy.scan)}</span>
            </div>
          ) : null}

          {scanState.status === "cancelled" ? (
            <div className="external-import__state is-muted" role="status" aria-live="polite">
              <XCircle size={18} aria-hidden="true" />
              <span>{extCopy.action.scanCancelled}</span>
            </div>
          ) : null}

          {scanState.status === "completed" && batchId ? (
            <ExternalImportSelectionPanel workflow={selectionWorkflow} />
          ) : null}
          </>
          )}
        </div>
      </Dialog>
    </>
  );
}
