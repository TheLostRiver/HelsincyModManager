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
  | {
      status: "starting";
      operation: ManagedInstallTaskOperation;
      profileId: string;
      modId: string;
      modName: string;
    }
  | {
      status: "running";
      operation: ManagedInstallTaskOperation;
      taskId: string;
      profileId: string;
      modId: string;
      modName: string;
      phase: ManagedInstallTaskPhase;
    }
  | {
      status: "completed";
      operation: ManagedInstallTaskOperation;
      taskId: string;
      profileId: string;
      modId: string;
      modName: string;
      phase: "install.completed" | "install.uninstall.completed";
    }
  | {
      status: "failed";
      operation: ManagedInstallTaskOperation;
      taskId: string | null;
      profileId: string;
      modId: string;
      modName: string;
      phase: "install.failed" | "install.uninstall.failed";
      message: string;
    }
  | {
      status: "cancelled";
      operation: "install";
      taskId: string;
      profileId: string;
      modId: string;
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

const installFailureMessages: Record<string, string> = {
  planning: "无法生成安装计划",
  lock: "安装任务暂时无法开始",
  commit: "安装未完成，已重新检查安装状态",
  complete: "安装收尾未完成，已重新检查安装状态",
  recovery_pending: "安装被待处理的恢复状态阻断",
  recovery_unavailable: "安装状态暂时无法确认",
};

const uninstallFailureMessages: Record<string, string> = {
  lock: "卸载任务暂时无法开始",
  uninstall: "卸载未完成，已重新检查安装状态",
  complete: "卸载收尾未完成，已重新检查安装状态",
  recovery_pending: "卸载被待处理的恢复状态阻断",
  recovery_unavailable: "卸载状态暂时无法确认",
};

export function getManagedInstallTaskFailureMessage(
  operation: ManagedInstallTaskOperation,
  error: string | null | undefined,
) {
  const prefix = operation === "uninstall" ? "install_uninstall_failed:" : "install_failed:";
  if (!error?.startsWith(prefix)) {
    return defaultManagedInstallTaskErrorMessage(operation);
  }

  const failurePhase = error.slice(prefix.length);
  const messages = operation === "uninstall" ? uninstallFailureMessages : installFailureMessages;
  return messages[failurePhase] ?? defaultManagedInstallTaskErrorMessage(operation);
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
      profileId: current.profileId,
      modId: current.modId,
      modName: current.modName,
      phase,
    };
  }

  if (isFailedPhase(phase)) {
    return {
      status: "failed",
      operation,
      taskId: event.taskId,
      profileId: current.profileId,
      modId: current.modId,
      modName: current.modName,
      phase,
      message: getManagedInstallTaskFailureMessage(operation, event.error),
    };
  }

  if (phase === "install.cancelled") {
    return {
      status: "cancelled",
      operation: "install",
      taskId: event.taskId,
      profileId: current.profileId,
      modId: current.modId,
      modName: current.modName,
      phase,
    };
  }

  return {
    status: "running",
    operation,
    taskId: event.taskId,
    profileId: current.profileId,
    modId: current.modId,
    modName: current.modName,
    phase,
  };
}
