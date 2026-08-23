import type { TaskProgressEventDto } from "./modImportTypes";
import type { ModLifecycleCopy } from "./modLifecycleCopy";

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

// 阶段集合只承担语义判断；文本一律经 modLifecycleCopy.installTask 取。
const managedInstallTaskPhases: ReadonlySet<string> = new Set([
  "install.queued",
  "install.plan.building",
  "install.commit.processing",
  "install.completed",
  "install.failed",
  "install.cancelled",
  "install.uninstall.queued",
  "install.uninstall.processing",
  "install.uninstall.completed",
  "install.uninstall.failed",
]);

export function isManagedInstallTaskPhase(phase: string): phase is ManagedInstallTaskPhase {
  return managedInstallTaskPhases.has(phase);
}

export function getManagedInstallTaskPhaseLabel(
  phase: ManagedInstallTaskPhase,
  installTask: ModLifecycleCopy["installTask"],
) {
  return installTask.phases[phase];
}

export function getManagedInstallTaskStartingLabel(
  operation: ManagedInstallTaskOperation,
  installTask: ModLifecycleCopy["installTask"],
) {
  return operation === "uninstall" ? installTask.startingUninstall : installTask.startingInstall;
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

export function defaultManagedInstallTaskErrorMessage(
  operation: ManagedInstallTaskOperation,
  installTask: ModLifecycleCopy["installTask"],
) {
  return operation === "uninstall"
    ? installTask.uninstallFailedDefault
    : installTask.installFailedDefault;
}

export function getManagedInstallTaskFailureMessage(
  operation: ManagedInstallTaskOperation,
  error: string | null | undefined,
  installTask: ModLifecycleCopy["installTask"],
) {
  const prefix = operation === "uninstall" ? "install_uninstall_failed:" : "install_failed:";
  if (!error?.startsWith(prefix)) {
    return defaultManagedInstallTaskErrorMessage(operation, installTask);
  }

  const failurePhase = error.slice(prefix.length);
  const messages = operation === "uninstall" ? installTask.uninstallFailures : installTask.installFailures;
  return messages[failurePhase] ?? defaultManagedInstallTaskErrorMessage(operation, installTask);
}

export function nextManagedInstallTaskStateFromProgress(
  current: ManagedInstallTaskState,
  event: TaskProgressEventDto,
  installTask: ModLifecycleCopy["installTask"],
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
      message: getManagedInstallTaskFailureMessage(operation, event.error, installTask),
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
