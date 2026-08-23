import type { TaskProgressEventDto } from "../mods/modImportTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";
import type { ReplacementCopy } from "./replacementCopy";

export type RetargetInstallTaskPhase =
  | "install.retarget.queued"
  | "install.retarget.plan.building"
  | "install.retarget.commit.processing"
  | "install.retarget.completed"
  | "install.retarget.failed"
  | "install.cancelled"
  | "install.reinstall.queued"
  | "install.reinstall.plan.building"
  | "install.reinstall.preflight.processing"
  | "install.reinstall.commit.processing"
  | "install.reinstall.rollback.processing"
  | "install.reinstall.completed"
  | "install.reinstall.failed"
  | "install.reinstall.cancelled";

export type RetargetInstallTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: RetargetInstallTaskPhase }
  | {
      status: "completed";
      taskId: string;
      phase: "install.retarget.completed" | "install.reinstall.completed";
    }
  | {
      status: "cancelled";
      taskId: string;
      phase: "install.cancelled" | "install.reinstall.cancelled";
    }
  | {
      status: "failed";
      taskId: string | null;
      phase: "install.retarget.failed" | "install.reinstall.failed";
      message: string;
    };

// 阶段集合只承担语义判断；文本一律经 replacementCopy.phases 取。
const retargetInstallTaskPhases: ReadonlySet<string> = new Set([
  "install.retarget.queued",
  "install.retarget.plan.building",
  "install.retarget.commit.processing",
  "install.retarget.completed",
  "install.retarget.failed",
  "install.cancelled",
  "install.reinstall.queued",
  "install.reinstall.plan.building",
  "install.reinstall.preflight.processing",
  "install.reinstall.commit.processing",
  "install.reinstall.rollback.processing",
  "install.reinstall.completed",
  "install.reinstall.failed",
  "install.reinstall.cancelled",
]);

export function isRetargetInstallTaskPhase(phase: string): phase is RetargetInstallTaskPhase {
  return retargetInstallTaskPhases.has(phase);
}

export function retargetInstallTaskPhaseLabel(
  phase: RetargetInstallTaskPhase,
  phases: ReplacementCopy["phases"],
) {
  return phases[phase];
}

export function canCancelRetargetInstallTaskPhase(phase: RetargetInstallTaskPhase) {
  return (
    phase === "install.retarget.queued" ||
    phase === "install.retarget.plan.building" ||
    phase === "install.reinstall.queued" ||
    phase === "install.reinstall.plan.building" ||
    phase === "install.reinstall.preflight.processing"
  );
}

export function nextRetargetInstallTaskState(
  current: RetargetInstallTaskState,
  event: Pick<TaskProgressEventDto, "taskId" | "kind" | "status" | "phase"> &
    Partial<Pick<TaskProgressEventDto, "error" | "message">>,
  events: ReplacementCopy["events"],
): RetargetInstallTaskState {
  if (
    current.status === "completed" ||
    current.status === "failed" ||
    current.status === "cancelled"
  ) {
    return current;
  }
  if (
    event.kind !== "install" ||
    !("taskId" in current) ||
    current.taskId === null ||
    current.taskId !== event.taskId ||
    !isRetargetInstallTaskPhase(event.phase)
  ) {
    return current;
  }

  if (
    event.status === "completed" &&
    (event.phase === "install.retarget.completed" || event.phase === "install.reinstall.completed")
  ) {
    return {
      status: "completed",
      taskId: event.taskId,
      phase: event.phase,
    };
  }
  if (
    event.status === "failed" &&
    (event.phase === "install.retarget.failed" || event.phase === "install.reinstall.failed")
  ) {
    return {
      status: "failed",
      taskId: event.taskId,
      phase: event.phase,
      message:
        event.phase === "install.reinstall.failed"
          ? events.reinstallFailed
          : events.retargetFailed,
    };
  }
  if (
    event.status === "cancelled" &&
    (event.phase === "install.cancelled" || event.phase === "install.reinstall.cancelled")
  ) {
    return {
      status: "cancelled",
      taskId: event.taskId,
      phase: event.phase,
    };
  }
  return {
    status: "running",
    taskId: event.taskId,
    phase: event.phase,
  };
}

export type RetargetInstallRefreshState =
  | { status: "idle" }
  | { status: "refreshing" }
  | { status: "ready" }
  | { status: "failed"; message: string };

export async function refreshRetargetInstallState(
  onInstallCompleted: () => Promise<void> | void,
  events: ReplacementCopy["events"],
): Promise<Extract<RetargetInstallRefreshState, { status: "ready" | "failed" }>> {
  try {
    await onInstallCompleted();
    return { status: "ready" };
  } catch {
    return {
      status: "failed",
      message: events.refreshFailed,
    };
  }
}

type InitialRetargetInstallAvailability = {
  installStatus: InstallManifestStatus | undefined;
  completedLocally: boolean;
  hasPreview: boolean;
  hasBlockingConflicts: boolean;
  prerequisiteStatus: "ready" | "warning" | "blocked";
  taskActive: boolean;
  listenerReady: boolean;
};

export function canStartInitialRetargetInstall(input: InitialRetargetInstallAvailability) {
  return (
    input.installStatus === "not_installed" &&
    !input.completedLocally &&
    input.hasPreview &&
    !input.hasBlockingConflicts &&
    input.prerequisiteStatus !== "blocked" &&
    !input.taskActive &&
    input.listenerReady
  );
}

type RetargetReinstallAvailability = {
  installStatus: InstallManifestStatus | undefined;
  previewStatus: "ready" | "blocked" | undefined;
  taskActive: boolean;
  listenerReady: boolean;
};

export function canStartRetargetReinstall(input: RetargetReinstallAvailability) {
  return (
    input.installStatus === "installed" &&
    input.previewStatus === "ready" &&
    !input.taskActive &&
    input.listenerReady
  );
}

export function resolveInstalledReplacementTargetSelection(
  targets: readonly { id: string }[],
  installedTargetId: string | undefined,
) {
  return installedTargetId !== undefined && targets.some((target) => target.id === installedTargetId)
    ? installedTargetId
    : null;
}

export function isCurrentInstalledReplacementTarget(
  targetId: string,
  installedTargetId: string | undefined,
) {
  return installedTargetId !== undefined && targetId === installedTargetId;
}
