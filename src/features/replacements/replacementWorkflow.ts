import type { TaskProgressEventDto } from "../mods/modImportTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";

export type RetargetInstallTaskPhase =
  | "install.retarget.queued"
  | "install.retarget.plan.building"
  | "install.retarget.commit.processing"
  | "install.retarget.completed"
  | "install.retarget.failed";

export type RetargetInstallTaskState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: RetargetInstallTaskPhase }
  | { status: "completed"; taskId: string; phase: "install.retarget.completed" }
  | {
      status: "failed";
      taskId: string | null;
      phase: "install.retarget.failed";
      message: string;
    };

const phaseLabels: Record<RetargetInstallTaskPhase, string> = {
  "install.retarget.queued": "等待安装",
  "install.retarget.plan.building": "重建替换计划",
  "install.retarget.commit.processing": "写入并记录安装清单",
  "install.retarget.completed": "替换目标安装完成",
  "install.retarget.failed": "替换目标安装失败",
};

export function isRetargetInstallTaskPhase(phase: string): phase is RetargetInstallTaskPhase {
  return Object.prototype.hasOwnProperty.call(phaseLabels, phase);
}

export function retargetInstallTaskPhaseLabel(phase: RetargetInstallTaskPhase) {
  return phaseLabels[phase];
}

export function nextRetargetInstallTaskState(
  current: RetargetInstallTaskState,
  event: Pick<TaskProgressEventDto, "taskId" | "kind" | "status" | "phase"> &
    Partial<Pick<TaskProgressEventDto, "error" | "message">>,
): RetargetInstallTaskState {
  if (
    event.kind !== "install" ||
    !("taskId" in current) ||
    current.taskId === null ||
    current.taskId !== event.taskId ||
    !isRetargetInstallTaskPhase(event.phase)
  ) {
    return current;
  }

  if (event.phase === "install.retarget.completed") {
    return {
      status: "completed",
      taskId: event.taskId,
      phase: event.phase,
    };
  }
  if (event.phase === "install.retarget.failed") {
    return {
      status: "failed",
      taskId: event.taskId,
      phase: event.phase,
      message: event.error ?? event.message ?? "替换目标安装失败",
    };
  }
  return {
    status: "running",
    taskId: event.taskId,
    phase: event.phase,
  };
}

type InitialRetargetInstallAvailability = {
  installStatus: InstallManifestStatus | undefined;
  completedLocally: boolean;
  hasPreview: boolean;
  hasBlockingConflicts: boolean;
  taskActive: boolean;
  listenerReady: boolean;
};

export function canStartInitialRetargetInstall(input: InitialRetargetInstallAvailability) {
  return (
    input.installStatus === "not_installed" &&
    !input.completedLocally &&
    input.hasPreview &&
    !input.hasBlockingConflicts &&
    !input.taskActive &&
    input.listenerReady
  );
}
