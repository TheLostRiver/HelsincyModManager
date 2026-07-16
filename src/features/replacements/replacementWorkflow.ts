import type { TaskProgressEventDto } from "../mods/modImportTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";

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

const phaseLabels: Record<RetargetInstallTaskPhase, string> = {
  "install.retarget.queued": "等待安装",
  "install.retarget.plan.building": "重建替换计划",
  "install.retarget.commit.processing": "写入并记录安装清单",
  "install.retarget.completed": "替换目标安装完成",
  "install.retarget.failed": "替换目标安装失败",
  "install.cancelled": "替换目标安装已取消",
  "install.reinstall.queued": "等待目标切换",
  "install.reinstall.plan.building": "生成目标切换计划",
  "install.reinstall.preflight.processing": "执行提交前检查",
  "install.reinstall.commit.processing": "提交目标切换",
  "install.reinstall.rollback.processing": "恢复原目标",
  "install.reinstall.completed": "替换目标切换完成",
  "install.reinstall.failed": "替换目标切换失败",
  "install.reinstall.cancelled": "替换目标切换已取消",
};

export function isRetargetInstallTaskPhase(phase: string): phase is RetargetInstallTaskPhase {
  return Object.prototype.hasOwnProperty.call(phaseLabels, phase);
}

export function retargetInstallTaskPhaseLabel(phase: RetargetInstallTaskPhase) {
  return phaseLabels[phase];
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
          ? "目标切换失败，请刷新状态并重新生成预览"
          : "替换目标安装失败，请刷新状态后重试",
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
): Promise<Extract<RetargetInstallRefreshState, { status: "ready" | "failed" }>> {
  try {
    await onInstallCompleted();
    return { status: "ready" };
  } catch {
    return {
      status: "failed",
      message: "安装已完成，但状态刷新失败，请重试。",
    };
  }
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
