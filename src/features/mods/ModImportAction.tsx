import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FileArchive, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import { modImportCopy, type ModImportCopy } from "./modImportCopy";
import { startImportModRevisionTask, startImportModTask } from "./modImportApi";
import {
  TASK_PROGRESS_EVENT_NAME,
  type TaskProgressEventDto,
} from "./modImportTypes";
import {
  consumeReconnectImportRequest,
  getModImportFailedMessage,
  getModImportTaskPhaseLabel,
  nextModImportTaskStateFromProgress,
  type ModImportFailedMessageKind,
  type ModImportTaskState,
} from "./modImportTaskState";
import "./ModImportAction.css";

type ModImportActionProps = {
  label: string;
  mode?: "new" | "revision";
  modId?: string | null;
  disabledReason?: string;
  tourId?: string;
  onImported: () => Promise<void> | void;
};

function startErrorMessageKind(error: unknown): ModImportFailedMessageKind {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String(error.code)
      : "unknown";

  if (code === "archive_path_empty" || code === "archive_path_not_absolute") {
    return "invalid-archive";
  }
  return "start-failed";
}

function isImportTaskActive(state: ModImportTaskState) {
  return state.status === "choosing" || state.status === "starting" || state.status === "running";
}

function isImportTaskTerminal(state: ModImportTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

function importActionLabel(
  label: string,
  state: ModImportTaskState,
  mode: "new" | "revision",
  copy: ModImportCopy,
) {
  switch (state.status) {
    case "choosing":
      return copy.action.pickArchive;
    case "starting":
      return copy.action.starting;
    case "running":
      return getModImportTaskPhaseLabel(state.phase, copy.phases);
    case "completed":
      return mode === "revision" ? copy.action.continueRevision : copy.action.continueImport;
    case "failed":
    case "cancelled":
      return mode === "revision" ? copy.action.retryRevision : copy.action.retryImport;
    default:
      return label;
  }
}

function importStatusText(state: ModImportTaskState, mode: "new" | "revision", copy: ModImportCopy) {
  switch (state.status) {
    case "choosing":
      return copy.status.waitingArchive;
    case "starting":
      return copy.status.creatingTask;
    case "running":
      return getModImportTaskPhaseLabel(state.phase, copy.phases);
    case "completed":
      return mode === "revision" ? copy.status.revisionDone : copy.status.importDone;
    case "cancelled":
      return copy.status.cancelled;
    case "failed":
      return getModImportFailedMessage(state.messageKind, copy);
    default:
      return null;
  }
}

export function ModImportAction({
  label,
  mode = "new",
  modId,
  disabledReason,
  tourId,
  onImported,
}: ModImportActionProps) {
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(modImportCopy, locale);
  const statusId = useId();
  const [listenerStatus, setListenerStatus] = useState<"loading" | "ready" | "failed">("loading");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [taskState, setTaskState] = useState<ModImportTaskState>({ status: "idle" });
  const taskStateRef = useRef<ModImportTaskState>(taskState);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const completedTaskIdsRef = useRef(new Set<string>());
  const continueImportAfterReconnectRef = useRef(false);
  const handleImportRef = useRef<() => Promise<void>>(async () => undefined);
  const onImportedRef = useRef(onImported);
  const displayedTaskNoticeIdRef = useRef<string | null>(null);

  const setTrackedTaskState = useCallback((next: ModImportTaskState) => {
    taskStateRef.current = next;
    setTaskState(next);
  }, []);

  const finishCompletedImport = useCallback((state: ModImportTaskState) => {
    if (state.status !== "completed" || completedTaskIdsRef.current.has(state.taskId)) {
      return;
    }

    completedTaskIdsRef.current.add(state.taskId);
    void Promise.resolve()
      .then(() => onImportedRef.current())
      .then(
        () => pushToast({
          eventKey: `mod-import.completed.${state.taskId}`,
          taskId: state.taskId,
          title: mode === "revision" ? copy.toasts.revisionDoneTitle : copy.toasts.importDoneTitle,
          message: mode === "revision" ? copy.toasts.revisionDoneMessage : copy.toasts.importDoneMessage,
          tone: "success",
        }),
        () => pushToast({
          eventKey: `mod-import.refresh-failed.${state.taskId}`,
          taskId: state.taskId,
          title: copy.toasts.refreshFailedTitle,
          message: copy.toasts.refreshFailedMessage,
          tone: "warning",
        }),
      );
  }, [copy, mode, pushToast]);

  const applyProgressState = useCallback(
    (next: ModImportTaskState) => {
      if (isImportTaskTerminal(next)) {
        taskIdRef.current = null;
      }
      setTrackedTaskState(next);
      finishCompletedImport(next);
    },
    [finishCompletedImport, setTrackedTaskState],
  );

  useEffect(() => {
    onImportedRef.current = onImported;
  }, [onImported]);

  useEffect(() => {
    handleImportRef.current = handleImport;
  });

  useEffect(() => {
    const reconnect = consumeReconnectImportRequest(
      listenerStatus,
      continueImportAfterReconnectRef.current,
    );
    continueImportAfterReconnectRef.current = reconnect.nextRequested;
    if (reconnect.shouldStart) void handleImportRef.current();
  }, [listenerStatus]);

  useEffect(() => {
    if (taskState.status === "failed") {
      pushToast({
        eventKey: `mod-import.failed.${taskState.taskId ?? taskState.phase}`,
        taskId: taskState.taskId ?? undefined,
        title: copy.toasts.importFailedTitle,
        message: getModImportFailedMessage(taskState.messageKind, copy),
        tone: "danger",
      });
    } else if (taskState.status === "cancelled") {
      pushToast({
        eventKey: `mod-import.cancelled.${taskState.taskId}`,
        taskId: taskState.taskId,
        title: copy.toasts.importCancelledTitle,
        message: copy.toasts.importCancelledMessage,
        tone: "neutral",
      });
    }
  }, [copy, pushToast, taskState]);

  useEffect(() => {
    const previousTaskId = displayedTaskNoticeIdRef.current;
    if (taskState.status === "running") {
      if (previousTaskId && previousTaskId !== taskState.taskId) dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = taskState.taskId;
      showTaskNotice({
        taskId: taskState.taskId,
        title: mode === "revision" ? copy.toasts.importingRevisionTitle : copy.toasts.importingTitle,
        message: importStatusText(taskState, mode, copy) ?? copy.status.running,
        tone: "progress",
      });
      return;
    }
    if (previousTaskId) {
      dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = null;
    }
  }, [copy, dismissTaskNotice, mode, showTaskNotice, taskState]);

  useEffect(() => () => {
    const taskId = displayedTaskNoticeIdRef.current;
    if (taskId) dismissTaskNotice(taskId);
  }, [dismissTaskNotice]);

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

      const next = nextModImportTaskStateFromProgress(taskStateRef.current, event.payload);
      applyProgressState(next);
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
          setTrackedTaskState({
            status: "failed",
            taskId: null,
            phase: "mod_import.listener.failed",
            messageKind: "listener-unavailable",
          });
        }
      });

    return () => {
      disposed = true;
      unlistenTaskProgress?.();
    };
  }, [applyProgressState, listenerAttempt, setTrackedTaskState]);

  function retryTaskProgressListener() {
    if (listenerStatus !== "failed") {
      return;
    }

    setListenerStatus("loading");
    setTrackedTaskState({ status: "idle" });
    setListenerAttempt((attempt) => attempt + 1);
  }

  async function handleImport() {
    if (
      listenerStatus !== "ready" ||
      isImportTaskActive(taskStateRef.current) ||
      disabledReason ||
      (mode === "revision" && !modId)
    ) {
      return;
    }

    taskIdRef.current = null;
    setTrackedTaskState({ status: "choosing" });

    let selected: string | string[] | null;
    try {
      selected = await open({
        directory: false,
        multiple: false,
        title: mode === "revision" ? copy.dialog.revisionTitle : copy.dialog.newTitle,
        filters: [{ name: copy.dialog.zipFilterName, extensions: ["zip"] }],
      });
    } catch {
      setTrackedTaskState({
        status: "failed",
        taskId: null,
        phase: "mod_import.picker.failed",
        messageKind: "picker-failed",
      });
      return;
    }

    if (typeof selected !== "string") {
      setTrackedTaskState({ status: "idle" });
      return;
    }

    startPendingRef.current = true;
    pendingProgressEventsRef.current.clear();
    setTrackedTaskState({ status: "starting" });

    try {
      const task =
        mode === "revision" && modId
          ? await startImportModRevisionTask({ archivePath: selected, modId })
          : await startImportModTask({ archivePath: selected });
      startPendingRef.current = false;

      if (task.kind !== "mod_import" || task.status !== "queued") {
        pendingProgressEventsRef.current.clear();
        setTrackedTaskState({
          status: "failed",
          taskId: null,
          phase: "mod_import.start.failed",
          messageKind: "invalid-start-state",
        });
        return;
      }

      taskIdRef.current = task.taskId;
      let next: ModImportTaskState = {
        status: "running",
        taskId: task.taskId,
        phase: "mod_import.queued",
      };
      const pendingProgressEvent = pendingProgressEventsRef.current.get(task.taskId);
      pendingProgressEventsRef.current.clear();
      if (pendingProgressEvent) {
        next = nextModImportTaskStateFromProgress(next, pendingProgressEvent);
      }
      applyProgressState(next);
    } catch (error: unknown) {
      startPendingRef.current = false;
      pendingProgressEventsRef.current.clear();
      setTrackedTaskState({
        status: "failed",
        taskId: null,
        phase: "mod_import.start.failed",
        messageKind: startErrorMessageKind(error),
      });
    }
  }

  const taskActive = isImportTaskActive(taskState);
  const statusText = disabledReason ?? (listenerStatus === "failed"
    ? copy.status.listenerFailedHint
    : undefined);
  const listenerLoading = listenerStatus === "loading";
  const actionDisabled =
    listenerLoading || taskActive || Boolean(disabledReason) || (mode === "revision" && !modId);

  return (
    <>
      {/* 禁用原因与服务状态走 tooltip（与其他工具栏按钮一致），不再以内联
          红字占据工具栏；服务待重连时按钮加警示点，可访问播报由隐藏的
          live region 保留。 */}
      <ModLibraryControlTooltip content={statusText}>
        {(descriptionId) => (
          <button
            type="button"
            className="compact-action compact-import-action is-primary"
            data-variant="primary"
            data-listener-status={listenerStatus}
            data-tour-id={tourId}
            onClick={() => {
              if (listenerStatus === "failed") {
                continueImportAfterReconnectRef.current = true;
                retryTaskProgressListener();
                return;
              }
              void handleImport();
            }}
            disabled={actionDisabled}
            aria-describedby={statusText ? descriptionId : undefined}
          >
            <span className="compact-action__left">
              {taskActive || listenerLoading ? (
                <LoaderCircle className="compact-import-action__spinner" size={14} aria-hidden="true" />
              ) : (
                <FileArchive size={14} strokeWidth={2.6} aria-hidden="true" />
              )}
              <span className="compact-action__label">
                {listenerLoading
                  ? copy.action.preparing
                  : listenerStatus === "failed"
                    ? mode === "revision" ? copy.action.reconnectRevision : copy.action.reconnectImport
                    : importActionLabel(label, taskState, mode, copy)}
              </span>
              {listenerStatus === "failed" ? (
                <span className="compact-import-action__alert-dot" aria-hidden="true" />
              ) : null}
            </span>
          </button>
        )}
      </ModLibraryControlTooltip>

      <span
        id={statusId}
        className="compact-import-action__sr-status"
        role={taskState.status === "failed" ? "alert" : "status"}
        aria-live="polite"
        aria-atomic="true"
      >
        {statusText ?? ""}
      </span>
    </>
  );
}
