import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { GameId } from "../game-setup/gameSetupTypes";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";
import {
  previewRecoveryAction,
  startRecoveryActionTask,
} from "../mods/modInstallPlanApi";
import type {
  InstallRecoveryActionPreview,
} from "../mods/modInstallPlanTypes";
import {
  TASK_PROGRESS_EVENT_NAME,
  type TaskProgressEventDto,
} from "../mods/modImportTypes";
import { notifyInstallRecoveryRefresh } from "./installRecoveryRefresh";

export type RecoveryRollbackPhase =
  | "install.recovery.queued"
  | "install.recovery.planning"
  | "install.recovery.processing"
  | "install.recovery.completed"
  | "install.recovery.failed";

// failed 只存语义 reason 与后端透传消息，文本在渲染时经 recoveryCenterCopy 取。
export type RecoveryRollbackFailureReason =
  | "profile_not_ready"
  | "preview_failed"
  | "start_failed"
  | "task_failed";

export type RecoveryRollbackState =
  | { status: "idle" }
  | { status: "previewing"; modId: string }
  | { status: "blocked"; modId: string; preview: InstallRecoveryActionPreview }
  | { status: "confirming"; modId: string; preview: InstallRecoveryActionPreview }
  | { status: "starting"; modId: string }
  | { status: "running"; modId: string; taskId: string; phase: RecoveryRollbackPhase }
  | { status: "completed"; modId: string; taskId: string }
  | { status: "failed"; modId: string; reason: RecoveryRollbackFailureReason; backendMessage: string | null };

// 语义 Set：阶段判定不依赖任何文案表。
const recoveryRollbackPhases: ReadonlySet<string> = new Set<RecoveryRollbackPhase>([
  "install.recovery.queued",
  "install.recovery.planning",
  "install.recovery.processing",
  "install.recovery.completed",
  "install.recovery.failed",
]);

export function getRecoveryRollbackPhaseLabel(
  phase: RecoveryRollbackPhase,
  phaseLabels: Record<RecoveryRollbackPhase, string>,
) {
  return phaseLabels[phase];
}

function isRecoveryRollbackPhase(phase: string): phase is RecoveryRollbackPhase {
  return recoveryRollbackPhases.has(phase);
}

type UseRecoveryRollbackInput = {
  gameId: GameId;
  onCompleted: () => void;
};

export function useRecoveryRollback(input: UseRecoveryRollbackInput) {
  const { gameId, onCompleted } = input;
  const { activeProfile, activeProfileId } = useActiveProfile();
  const [state, setState] = useState<RecoveryRollbackState>({ status: "idle" });
  const stateRef = useRef(state);
  stateRef.current = state;

  const onCompletedRef = useRef(onCompleted);
  onCompletedRef.current = onCompleted;

  const pendingEventsRef = useRef(new Map<string, TaskProgressEventDto>());

  const markCompleted = useCallback((modId: string, taskId: string) => {
    setState({ status: "completed", modId, taskId });
    notifyInstallRecoveryRefresh();
    onCompletedRef.current();
  }, []);

  const requestRollback = useCallback(
    (modId: string) => {
      if (stateRef.current.status !== "idle") {
        return;
      }

      if (activeProfile.status !== "ready" || activeProfileId === null) {
        setState({ status: "failed", modId, reason: "profile_not_ready", backendMessage: null });
        return;
      }

      setState({ status: "previewing", modId });

      void previewRecoveryAction({
        gameId,
        profileId: activeProfileId,
        modId,
        actionKind: "rollback_install",
      })
        .then((preview) => {
          if (stateRef.current.status !== "previewing" || stateRef.current.modId !== modId) {
            return;
          }

          if (preview.availability === "available") {
            setState({ status: "confirming", modId, preview });
          } else {
            setState({ status: "blocked", modId, preview });
          }
        })
        .catch(() => {
          if (stateRef.current.status === "previewing" && stateRef.current.modId === modId) {
            setState({ status: "failed", modId, reason: "preview_failed", backendMessage: null });
          }
        });
    },
    [activeProfile.status, activeProfileId, gameId],
  );

  const confirmRollback = useCallback(() => {
    const current = stateRef.current;
    if (current.status !== "confirming") {
      return;
    }

    const { modId } = current;
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      setState({ status: "failed", modId, reason: "profile_not_ready", backendMessage: null });
      return;
    }

    setState({ status: "starting", modId });

    void startRecoveryActionTask({
      gameId,
      profileId: activeProfileId,
      modId,
      actionKind: "rollback_install",
    })
      .then((result) => {
        if (stateRef.current.status !== "starting" || stateRef.current.modId !== modId) {
          return;
        }

        const pending = pendingEventsRef.current.get(result.taskId);
        pendingEventsRef.current.clear();

        if (pending && isRecoveryRollbackPhase(pending.phase)) {
          if (pending.phase === "install.recovery.completed") {
            markCompleted(modId, result.taskId);
          } else if (pending.phase === "install.recovery.failed") {
            setState({
              status: "failed",
              modId,
              reason: "task_failed",
              backendMessage: pending.error ?? pending.message,
            });
          } else {
            setState({
              status: "running",
              modId,
              taskId: result.taskId,
              phase: pending.phase,
            });
          }
        } else {
          setState({
            status: "running",
            modId,
            taskId: result.taskId,
            phase: "install.recovery.queued",
          });
        }
      })
      .catch(() => {
        if (stateRef.current.status === "starting" && stateRef.current.modId === modId) {
          setState({ status: "failed", modId, reason: "start_failed", backendMessage: null });
        }
      });
  }, [activeProfile.status, activeProfileId, gameId, markCompleted]);

  const dismiss = useCallback(() => {
    setState({ status: "idle" });
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenFn: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed) {
        return;
      }

      if (event.payload.kind !== "install") {
        return;
      }

      const phase = event.payload.phase;
      if (!isRecoveryRollbackPhase(phase)) {
        return;
      }

      const current = stateRef.current;

      if (current.status === "starting") {
        pendingEventsRef.current.set(event.payload.taskId, event.payload);
        return;
      }

      if (current.status !== "running" || current.taskId !== event.payload.taskId) {
        return;
      }

      if (phase === "install.recovery.completed") {
        markCompleted(current.modId, current.taskId);
      } else if (phase === "install.recovery.failed") {
        setState({
          status: "failed",
          modId: current.modId,
          reason: "task_failed",
          backendMessage: event.payload.error ?? event.payload.message,
        });
      } else {
        setState({
          status: "running",
          modId: current.modId,
          taskId: current.taskId,
          phase,
        });
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }
      unlistenFn = unlisten;
    });

    return () => {
      disposed = true;
      unlistenFn?.();
    };
  }, [markCompleted]);

  return {
    state,
    requestRollback,
    confirmRollback,
    dismiss,
  };
}
