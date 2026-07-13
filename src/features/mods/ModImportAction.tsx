import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FileArchive, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { startImportModTask } from "./modImportApi";
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

function importActionLabel(label: string, state: ModImportTaskState) {
  switch (state.status) {
    case "choosing":
      return "选择压缩包...";
    case "starting":
      return "启动导入...";
    case "running":
      return getModImportTaskPhaseLabel(state.phase);
    case "completed":
      return "继续添加 MOD";
    case "failed":
    case "cancelled":
      return "重试添加 MOD";
    default:
      return label;
  }
}

function importStatusText(state: ModImportTaskState) {
  switch (state.status) {
    case "choosing":
      return "等待选择 ZIP 压缩包";
    case "starting":
      return "正在创建导入任务";
    case "running":
      return getModImportTaskPhaseLabel(state.phase);
    case "completed":
      return "导入完成，Mod 列表将自动刷新";
    case "cancelled":
      return "导入已取消";
    case "failed":
      return state.message;
    default:
      return null;
  }
}

export function ModImportAction({ label, onImported }: ModImportActionProps) {
  const [listenerStatus, setListenerStatus] = useState<"loading" | "ready" | "failed">("loading");
  const [taskState, setTaskState] = useState<ModImportTaskState>({ status: "idle" });
  const taskStateRef = useRef<ModImportTaskState>(taskState);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const completedTaskIdsRef = useRef(new Set<string>());
  const onImportedRef = useRef(onImported);

  const setTrackedTaskState = useCallback((next: ModImportTaskState) => {
    taskStateRef.current = next;
    setTaskState(next);
  }, []);

  const finishCompletedImport = useCallback((state: ModImportTaskState) => {
    if (state.status !== "completed" || completedTaskIdsRef.current.has(state.taskId)) {
      return;
    }

    completedTaskIdsRef.current.add(state.taskId);
    void Promise.resolve(onImportedRef.current()).catch(() => undefined);
  }, []);

  useEffect(() => {
    onImportedRef.current = onImported;
  }, [onImported]);

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
      setTrackedTaskState(next);
      finishCompletedImport(next);
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
  }, [finishCompletedImport, setTrackedTaskState]);

  async function handleImport() {
    if (listenerStatus !== "ready" || isImportTaskActive(taskStateRef.current)) {
      return;
    }

    taskIdRef.current = null;
    setTrackedTaskState({ status: "choosing" });

    let selected: string | string[] | null;
    try {
      selected = await open({
        directory: false,
        multiple: false,
        title: "选择 Mod ZIP 压缩包",
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
      const task = await startImportModTask({ archivePath: selected });
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
      setTrackedTaskState(next);
      finishCompletedImport(next);
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
  const statusText = importStatusText(taskState);
  const listenerLoading = listenerStatus === "loading";

  return (
    <>
      <button
        type="button"
        className="compact-action compact-import-action is-primary"
        data-variant="primary"
        onClick={() => void handleImport()}
        disabled={listenerStatus !== "ready" || taskActive}
        aria-describedby={statusText ? "mod-import-status" : undefined}
      >
        <span className="compact-action__left">
          {taskActive || listenerLoading ? (
            <LoaderCircle className="compact-import-action__spinner" size={14} aria-hidden="true" />
          ) : (
            <FileArchive size={14} strokeWidth={2.6} aria-hidden="true" />
          )}
          <span className="compact-action__label">
            {listenerLoading ? "准备导入..." : importActionLabel(label, taskState)}
          </span>
        </span>
      </button>

      {statusText ? (
        <span
          id="mod-import-status"
          className={`compact-import-action__status is-${taskState.status}`}
          role={taskState.status === "failed" ? "alert" : "status"}
        >
          {statusText}
        </span>
      ) : null}
    </>
  );
}
