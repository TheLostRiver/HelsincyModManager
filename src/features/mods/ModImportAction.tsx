import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FileArchive, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { startImportModRevisionTask, startImportModTask } from "./modImportApi";
import {
  TASK_PROGRESS_EVENT_NAME,
  type TaskProgressEventDto,
} from "./modImportTypes";
import {
  getModImportTaskPhaseLabel,
  nextModImportTaskStateFromProgress,
  type ModImportTaskState,
} from "./modImportTaskState";
import "./ModImportAction.css";

type ModImportActionProps = {
  label: string;
  mode?: "new" | "revision";
  modId?: string | null;
  disabledReason?: string;
  onImported: () => Promise<void> | void;
};

function startErrorMessage(error: unknown) {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String(error.code)
      : "unknown";

  if (code === "archive_path_empty" || code === "archive_path_not_absolute") {
    return "请选择有效的本地 ZIP 压缩包";
  }
  return "无法启动导入任务";
}

function isImportTaskActive(state: ModImportTaskState) {
  return state.status === "choosing" || state.status === "starting" || state.status === "running";
}

function isImportTaskTerminal(state: ModImportTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

function importActionLabel(label: string, state: ModImportTaskState, mode: "new" | "revision") {
  switch (state.status) {
    case "choosing":
      return "选择压缩包...";
    case "starting":
      return "启动导入...";
    case "running":
      return getModImportTaskPhaseLabel(state.phase);
    case "completed":
      return mode === "revision" ? "继续导入新版本" : "继续添加 MOD";
    case "failed":
    case "cancelled":
      return mode === "revision" ? "重试导入新版本" : "重试添加 MOD";
    default:
      return label;
  }
}

function importStatusText(state: ModImportTaskState, mode: "new" | "revision") {
  switch (state.status) {
    case "choosing":
      return "等待选择 ZIP 压缩包";
    case "starting":
      return "正在创建导入任务";
    case "running":
      return getModImportTaskPhaseLabel(state.phase);
    case "completed":
      return mode === "revision" ? "新版本导入完成，版本列表已更新" : "导入完成，Mod 列表将自动刷新";
    case "cancelled":
      return "导入已取消";
    case "failed":
      return state.message;
    default:
      return null;
  }
}

export function ModImportAction({
  label,
  mode = "new",
  modId,
  disabledReason,
  onImported,
}: ModImportActionProps) {
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();
  const statusId = useId();
  const [listenerStatus, setListenerStatus] = useState<"loading" | "ready" | "failed">("loading");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [taskState, setTaskState] = useState<ModImportTaskState>({ status: "idle" });
  const taskStateRef = useRef<ModImportTaskState>(taskState);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const completedTaskIdsRef = useRef(new Set<string>());
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
          title: mode === "revision" ? "新版本导入完成" : "Mod 导入完成",
          message: mode === "revision" ? "版本列表已更新。" : "Mod 列表已更新。",
          tone: "success",
        }),
        () => pushToast({
          eventKey: `mod-import.refresh-failed.${state.taskId}`,
          taskId: state.taskId,
          title: "导入完成，列表刷新失败",
          message: "文件已导入，但当前列表未能刷新，请重新扫描或稍后重试。",
          tone: "warning",
        }),
      );
  }, [mode, pushToast]);

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
    if (taskState.status === "failed") {
      pushToast({
        eventKey: `mod-import.failed.${taskState.taskId ?? taskState.phase}`,
        taskId: taskState.taskId ?? undefined,
        title: "Mod 导入失败",
        message: taskState.message,
        tone: "danger",
      });
    } else if (taskState.status === "cancelled") {
      pushToast({
        eventKey: `mod-import.cancelled.${taskState.taskId}`,
        taskId: taskState.taskId,
        title: "Mod 导入已取消",
        message: "未继续写入新的 Mod 版本。",
        tone: "neutral",
      });
    }
  }, [pushToast, taskState]);

  useEffect(() => {
    const previousTaskId = displayedTaskNoticeIdRef.current;
    if (taskState.status === "running") {
      if (previousTaskId && previousTaskId !== taskState.taskId) dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = taskState.taskId;
      showTaskNotice({
        taskId: taskState.taskId,
        title: mode === "revision" ? "正在导入新版本" : "正在导入 Mod",
        message: importStatusText(taskState, mode) ?? "正在执行导入任务",
        tone: "progress",
      });
      return;
    }
    if (previousTaskId) {
      dismissTaskNotice(previousTaskId);
      displayedTaskNoticeIdRef.current = null;
    }
  }, [dismissTaskNotice, mode, showTaskNotice, taskState]);

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
            message: "导入任务状态不可用",
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
        title: mode === "revision" ? "选择新版本 ZIP 压缩包" : "选择 Mod ZIP 压缩包",
        filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
      });
    } catch {
      setTrackedTaskState({
        status: "failed",
        taskId: null,
        phase: "mod_import.picker.failed",
        message: "无法打开文件选择器",
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
          message: "导入任务返回了无效状态",
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
        message: startErrorMessage(error),
      });
    }
  }

  const taskActive = isImportTaskActive(taskState);
  const statusText = disabledReason;
  const listenerLoading = listenerStatus === "loading";
  const actionDisabled =
    listenerLoading || taskActive || Boolean(disabledReason) || (mode === "revision" && !modId);

  return (
    <>
      <button
        type="button"
        className="compact-action compact-import-action is-primary"
        data-variant="primary"
        onClick={() => {
          if (listenerStatus === "failed") {
            retryTaskProgressListener();
            return;
          }
          void handleImport();
        }}
        disabled={actionDisabled}
        aria-describedby={statusText ? statusId : undefined}
      >
        <span className="compact-action__left">
          {taskActive || listenerLoading ? (
            <LoaderCircle className="compact-import-action__spinner" size={14} aria-hidden="true" />
          ) : (
            <FileArchive size={14} strokeWidth={2.6} aria-hidden="true" />
          )}
          <span className="compact-action__label">
            {listenerLoading
              ? "准备导入..."
              : listenerStatus === "failed"
                ? "重试导入连接"
                : importActionLabel(label, taskState, mode)}
          </span>
        </span>
      </button>

      {statusText ? (
        <span
          id={statusId}
          className={`compact-import-action__status is-${taskState.status}`}
          role={taskState.status === "failed" ? "alert" : "status"}
        >
          {statusText}
        </span>
      ) : null}
    </>
  );
}
