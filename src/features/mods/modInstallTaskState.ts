import type { TaskProgressEventDto } from "./modImportTypes";

export type ManagedInstallTaskOperation = "install" | "uninstall";

export type ManagedInstallTaskPhase =
  | "install.queued"
  | "install.plan.building"
  | "install.commit.processing"
  | "install.completed"
  | "install.failed"
  | "install.cancelled"
  | "install.uninstall.queued"
  | "install.uninstall.processing"
  | "install.uninstall.completed"
  | "install.uninstall.failed";

export type ManagedInstallTaskState =
  | { status: "idle" }
  | { status: "starting"; operation: ManagedInstallTaskOperation; modName: string }
  | {
      status: "running";
      operation: ManagedInstallTaskOperation;
      taskId: string;
      modName: string;
      phase: ManagedInstallTaskPhase;
    }
  | {
      status: "completed";
      operation: ManagedInstallTaskOperation;
      taskId: string;
      modName: string;
      phase: "install.completed" | "install.uninstall.completed";
    }
  | {
      status: "failed";
      operation: ManagedInstallTaskOperation;
      taskId: string | null;
      modName: string;
      phase: "install.failed" | "install.uninstall.failed";
      message: string;
    }
  | {
      status: "cancelled";
      operation: "install";
      taskId: string;
      modName: string;
      phase: "install.cancelled";
    };

export type ManagedInstallTaskStateUpdate =
  | ManagedInstallTaskState
  | ((current: ManagedInstallTaskState) => ManagedInstallTaskState);

const managedInstallTaskPhaseLabels: Record<ManagedInstallTaskPhase, string> = {
  "install.queued": "等待安装",
  "install.plan.building": "生成安装计划",
  "install.commit.processing": "写入中",
  "install.completed": "安装完成",
  "install.failed": "安装失败",
  "install.cancelled": "已取消",
  "install.uninstall.queued": "等待卸载",
  "install.uninstall.processing": "卸载中",
  "install.uninstall.completed": "卸载完成",
  "install.uninstall.failed": "卸载失败",
};

export function isManagedInstallTaskPhase(phase: string): phase is ManagedInstallTaskPhase {
  return Object.prototype.hasOwnProperty.call(managedInstallTaskPhaseLabels, phase);
}

export function getManagedInstallTaskPhaseLabel(phase: ManagedInstallTaskPhase) {
  return managedInstallTaskPhaseLabels[phase];
}

export function getManagedInstallTaskStartingLabel(operation: ManagedInstallTaskOperation) {
  return operation === "uninstall" ? "启动卸载任务" : "启动安装任务";
}

function isCompletedPhase(
  phase: ManagedInstallTaskPhase,
): phase is "install.completed" | "install.uninstall.completed" {
  return phase === "install.completed" || phase === "install.uninstall.completed";
}

function isFailedPhase(phase: ManagedInstallTaskPhase): phase is "install.failed" | "install.uninstall.failed" {
  return phase === "install.failed" || phase === "install.uninstall.failed";
}

export function operationForManagedInstallPhase(phase: ManagedInstallTaskPhase): ManagedInstallTaskOperation {
  return phase.startsWith("install.uninstall.") ? "uninstall" : "install";
}

export function defaultManagedInstallTaskErrorMessage(operation: ManagedInstallTaskOperation) {
  return operation === "uninstall" ? "卸载失败" : "安装失败";
}

export function nextManagedInstallTaskStateFromProgress(
  current: ManagedInstallTaskState,
  event: TaskProgressEventDto,
): ManagedInstallTaskState {
  if (event.kind !== "install" || !("taskId" in current) || current.taskId !== event.taskId) {
    return current;
  }

  const phase = event.phase;
  if (!isManagedInstallTaskPhase(phase)) {
    return current;
  }

  const operation = operationForManagedInstallPhase(phase);
  if (operation !== current.operation) {
    return current;
  }

  if (isCompletedPhase(phase)) {
    return {
      status: "completed",
      operation,
      taskId: event.taskId,
      modName: current.modName,
      phase,
    };
  }

  if (isFailedPhase(phase)) {
    return {
      status: "failed",
      operation,
      taskId: event.taskId,
      modName: current.modName,
      phase,
      message: event.error ?? event.message ?? defaultManagedInstallTaskErrorMessage(operation),
    };
  }

  if (phase === "install.cancelled") {
    return {
      status: "cancelled",
      operation: "install",
      taskId: event.taskId,
      modName: current.modName,
      phase,
    };
  }

  return {
    status: "running",
    operation,
    taskId: event.taskId,
    modName: current.modName,
    phase,
  };
}
