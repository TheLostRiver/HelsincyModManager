import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { GameId } from "../game-setup/gameSetupTypes";
import { getInstallManifestStatus } from "./modInstallPlanApi";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { getModRevisions } from "./modLibraryApi";
import type { ModLibraryItem, ModRevisionList } from "./modLibraryTypes";
import { modReinstallCopy } from "./modReinstallCopy";
import { previewReinstallPlan, startReinstallTask } from "./modReinstallApi";
import {
  canConfirmReinstall,
  canPreviewReinstall,
  getReinstallPreviewErrorMessage,
  getReinstallStartErrorMessage,
  isReinstallTaskPhase,
  isReinstallTaskTerminal,
  nextReinstallTaskStateFromProgress,
  refreshReinstallDurableFacts,
  type ReinstallTaskState,
} from "./modReinstallTaskState";
import type { ReinstallPlanPreview } from "./modReinstallTypes";

type ReinstallPreviewState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; preview: ReinstallPlanPreview }
  | { status: "error"; message: string };

export type ReinstallDialogState =
  | { status: "closed" }
  | {
      status: "open";
      gameId: GameId;
      profileId: string;
      modId: string;
      modName: string;
      installStatus: InstallManifestStatus;
      catalogStatus: "loading" | "ready" | "error";
      revisions: ModRevisionList | null;
      selectedCandidateRevisionId: string;
      previewState: ReinstallPreviewState;
      catalogMessage: string | null;
    };

type UseModReinstallWorkflowInput = {
  gameId: GameId;
  profileId: string | null;
  selectedItem: ModLibraryItem | null;
  writeTaskActive: boolean;
  refreshLibrary: () => Promise<void> | void;
};

type ReinstallTaskContext = {
  gameId: GameId;
  profileId: string;
  modId: string;
};

export function useModReinstallWorkflow({
  gameId,
  profileId,
  selectedItem,
  writeTaskActive,
  refreshLibrary,
}: UseModReinstallWorkflowInput) {
  const { locale } = useI18n();
  const reCopy = resolveCopy(modReinstallCopy, locale);
  // 事件监听回调经 ref 取词，避免语言切换导致监听器重建。
  const taskCopyRef = useRef(reCopy.task);
  taskCopyRef.current = reCopy.task;
  const [dialogState, setDialogState] = useState<ReinstallDialogState>({ status: "closed" });
  const dialogStateRef = useRef<ReinstallDialogState>(dialogState);
  const [taskState, setTaskState] = useState<ReinstallTaskState>({ status: "idle" });
  const taskStateRef = useRef<ReinstallTaskState>(taskState);
  const [listenerStatus, setListenerStatus] = useState<"loading" | "ready" | "failed">("loading");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingProgressEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const activeTaskContextRef = useRef<ReinstallTaskContext | null>(null);
  const refreshedTerminalTaskIdsRef = useRef(new Set<string>());
  const requestGenerationRef = useRef(0);

  const setTrackedDialogState = useCallback(
    (update: ReinstallDialogState | ((current: ReinstallDialogState) => ReinstallDialogState)) => {
      const next = typeof update === "function" ? update(dialogStateRef.current) : update;
      dialogStateRef.current = next;
      setDialogState(next);
    },
    [],
  );

  const setTrackedTaskState = useCallback((next: ReinstallTaskState) => {
    taskStateRef.current = next;
    setTaskState(next);
  }, []);

  const refreshTaskDurableFacts = useCallback(
    async (context: ReinstallTaskContext) => {
      const [facts] = await Promise.all([
        refreshReinstallDurableFacts({
          loadRevisions: () => getModRevisions({ modId: context.modId }),
          loadInstallStatus: async () => {
            const summaries = await getInstallManifestStatus({
              gameId: context.gameId,
              profileId: context.profileId,
              modIds: [context.modId],
            });
            return summaries.find((summary) => summary.modId === context.modId)?.status ?? "unknown";
          },
        }),
        Promise.resolve()
          .then(() => refreshLibrary())
          .catch(() => undefined),
      ]);

      setTrackedDialogState((current) => {
        if (current.status !== "open" || current.modId !== context.modId) {
          return current;
        }

        return {
          ...current,
          installStatus: facts.installStatus ?? "unknown",
          revisions: facts.revisions ?? current.revisions,
          catalogStatus: facts.revisions === null ? current.catalogStatus : "ready",
          catalogMessage: facts.revisions === null ? current.catalogMessage : null,
        };
      });
    },
    [refreshLibrary, setTrackedDialogState],
  );

  const applyProgressState = useCallback(
    (next: ReinstallTaskState) => {
      setTrackedTaskState(next);
      if (!isReinstallTaskTerminal(next)) {
        return;
      }

      taskIdRef.current = null;
      const context = activeTaskContextRef.current;
      if (!context || next.taskId === null || refreshedTerminalTaskIdsRef.current.has(next.taskId)) {
        return;
      }

      refreshedTerminalTaskIdsRef.current.add(next.taskId);
      void refreshTaskDurableFacts(context);
    },
    [refreshTaskDurableFacts, setTrackedTaskState],
  );
  const applyProgressStateRef = useRef(applyProgressState);

  useEffect(() => {
    applyProgressStateRef.current = applyProgressState;
  }, [applyProgressState]);

  useEffect(() => {
    let disposed = false;
    let unlistenTaskProgress: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (
        disposed ||
        event.payload.kind !== "install" ||
        !isReinstallTaskPhase(event.payload.phase)
      ) {
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

      const next = nextReinstallTaskStateFromProgress(
        taskStateRef.current,
        event.payload,
        taskCopyRef.current,
      );
      applyProgressStateRef.current(next);
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
  }, [listenerAttempt]);

  const retryTaskProgressListener = useCallback(() => {
    if (listenerStatus !== "failed") {
      return;
    }
    setListenerStatus("loading");
    setListenerAttempt((attempt) => attempt + 1);
  }, [listenerStatus]);

  const openReinstall = useCallback(() => {
    if (
      writeTaskActive ||
      profileId === null ||
      selectedItem === null ||
      selectedItem.installSummary?.status !== "installed"
    ) {
      return;
    }

    const generation = ++requestGenerationRef.current;
    setTrackedTaskState({ status: "idle" });
    setTrackedDialogState({
      status: "open",
      gameId,
      profileId,
      modId: selectedItem.id,
      modName: selectedItem.name,
      installStatus: "installed",
      catalogStatus: "loading",
      revisions: null,
      selectedCandidateRevisionId: "",
      previewState: { status: "idle" },
      catalogMessage: null,
    });

    void getModRevisions({ modId: selectedItem.id })
      .then((revisions) => {
        if (requestGenerationRef.current !== generation) {
          return;
        }
        const candidateRevisionId = revisions.displayRevisionId || revisions.revisions[0]?.revisionId || "";
        setTrackedDialogState((current) =>
          current.status === "open" && current.modId === selectedItem.id
            ? {
                ...current,
                catalogStatus: "ready",
                revisions,
                selectedCandidateRevisionId: candidateRevisionId,
                catalogMessage: candidateRevisionId ? null : reCopy.workflow.noCandidate,
              }
            : current,
        );
      })
      .catch(() => {
        if (requestGenerationRef.current !== generation) {
          return;
        }
        setTrackedDialogState((current) =>
          current.status === "open" && current.modId === selectedItem.id
            ? { ...current, catalogStatus: "error", catalogMessage: reCopy.workflow.catalogLoadFailed }
            : current,
        );
      });
  }, [gameId, profileId, reCopy, selectedItem, setTrackedDialogState, setTrackedTaskState, writeTaskActive]);

  const closeReinstall = useCallback(() => {
    const currentTask = taskStateRef.current;
    if (currentTask.status === "starting" || currentTask.status === "running") {
      return;
    }

    requestGenerationRef.current += 1;
    pendingProgressEventsRef.current.clear();
    startPendingRef.current = false;
    taskIdRef.current = null;
    activeTaskContextRef.current = null;
    setTrackedTaskState({ status: "idle" });
    setTrackedDialogState({ status: "closed" });
  }, [setTrackedDialogState, setTrackedTaskState]);

  const selectCandidateRevision = useCallback(
    (candidateRevisionId: string) => {
      requestGenerationRef.current += 1;
      setTrackedDialogState((current) =>
        current.status === "open"
          ? {
              ...current,
              selectedCandidateRevisionId: candidateRevisionId,
              previewState: { status: "idle" },
            }
          : current,
      );
    },
    [setTrackedDialogState],
  );

  const generatePreview = useCallback(() => {
    const current = dialogStateRef.current;
    const currentTask = taskStateRef.current;
    if (
      current.status !== "open" ||
      current.catalogStatus !== "ready" ||
      !canPreviewReinstall(current.installStatus, current.selectedCandidateRevisionId, currentTask)
    ) {
      return;
    }

    if (isReinstallTaskTerminal(currentTask)) {
      setTrackedTaskState({ status: "idle" });
    }
    const generation = ++requestGenerationRef.current;
    setTrackedDialogState({ ...current, previewState: { status: "loading" } });

    void previewReinstallPlan({
      gameId: current.gameId,
      profileId: current.profileId,
      modId: current.modId,
      candidateRevisionId: current.selectedCandidateRevisionId,
      layer: { name: "base", priority: 0 },
    })
      .then((preview) => {
        if (requestGenerationRef.current !== generation) {
          return;
        }
        setTrackedDialogState((latest) =>
          latest.status === "open" &&
          latest.modId === current.modId &&
          latest.selectedCandidateRevisionId === current.selectedCandidateRevisionId
            ? { ...latest, previewState: { status: "ready", preview } }
            : latest,
        );
      })
      .catch((error: unknown) => {
        if (requestGenerationRef.current !== generation) {
          return;
        }
        setTrackedDialogState((latest) =>
          latest.status === "open" && latest.modId === current.modId
            ? { ...latest, previewState: { status: "error", message: getReinstallPreviewErrorMessage(error, reCopy.task) } }
            : latest,
        );
      });
  }, [reCopy, setTrackedDialogState, setTrackedTaskState]);

  const confirmReinstall = useCallback(() => {
    const current = dialogStateRef.current;
    const currentTask = taskStateRef.current;
    if (
      listenerStatus !== "ready" ||
      current.status !== "open" ||
      current.previewState.status !== "ready" ||
      !canConfirmReinstall(current.installStatus, current.previewState.preview, currentTask)
    ) {
      return;
    }

    const preview = current.previewState.preview;
    if (preview.status !== "ready") {
      return;
    }

    const context: ReinstallTaskContext = {
      gameId: current.gameId,
      profileId: current.profileId,
      modId: current.modId,
    };
    activeTaskContextRef.current = context;
    startPendingRef.current = true;
    pendingProgressEventsRef.current.clear();
    setTrackedTaskState({
      status: "starting",
      modId: current.modId,
      modName: current.modName,
      candidateRevisionId: preview.candidateRevision.revisionId,
    });

    void startReinstallTask({
      gameId: current.gameId,
      profileId: current.profileId,
      modId: current.modId,
      candidateRevisionId: preview.candidateRevision.revisionId,
      layer: { name: "base", priority: 0 },
      planToken: preview.planToken,
    })
      .then((task) => {
        startPendingRef.current = false;
        const pendingProgressEvent = pendingProgressEventsRef.current.get(task.taskId) ?? null;
        pendingProgressEventsRef.current.clear();

        if (task.kind !== "install" || task.status !== "queued") {
          activeTaskContextRef.current = null;
          setTrackedTaskState({ status: "idle" });
          setTrackedDialogState((latest) =>
            latest.status === "open"
              ? { ...latest, previewState: { status: "error", message: reCopy.workflow.invalidTaskState } }
              : latest,
          );
          return;
        }

        taskIdRef.current = task.taskId;
        let next: ReinstallTaskState = {
          status: "running",
          taskId: task.taskId,
          modId: current.modId,
          modName: current.modName,
          candidateRevisionId: preview.candidateRevision.revisionId,
          phase: "install.reinstall.queued",
        };
        if (pendingProgressEvent) {
          next = nextReinstallTaskStateFromProgress(next, pendingProgressEvent, reCopy.task);
        }
        applyProgressState(next);
      })
      .catch((error: unknown) => {
        startPendingRef.current = false;
        pendingProgressEventsRef.current.clear();
        activeTaskContextRef.current = null;
        setTrackedTaskState({ status: "idle" });
        setTrackedDialogState((latest) =>
          latest.status === "open"
            ? { ...latest, previewState: { status: "error", message: getReinstallStartErrorMessage(error, reCopy.task) } }
            : latest,
        );
      });
  }, [applyProgressState, listenerStatus, reCopy, setTrackedDialogState, setTrackedTaskState]);

  const taskActive = taskState.status === "starting" || taskState.status === "running";
  const canConfirm =
    listenerStatus === "ready" &&
    dialogState.status === "open" &&
    dialogState.previewState.status === "ready" &&
    canConfirmReinstall(dialogState.installStatus, dialogState.previewState.preview, taskState);

  return {
    dialogState,
    taskState,
    listenerStatus,
    taskActive,
    workflowActive: dialogState.status === "open",
    canConfirm,
    openReinstall,
    closeReinstall,
    selectCandidateRevision,
    generatePreview,
    confirmReinstall,
    retryTaskProgressListener,
  };
}
