import type { ModInstallSummaryStatus, ModLibraryItem } from "./modLibraryTypes";
import type { ManagedInstallTaskOperation, ManagedInstallTaskState } from "./modInstallTaskState";
import type { ModLifecycleCopy } from "./modLifecycleCopy";

export type ModLifecycleToast = {
  id: string;
  title: string;
  message: string;
  tone: "neutral" | "success" | "danger";
};

export type ManagedInstallTerminalRefresh = {
  verified: boolean;
  status: ModInstallSummaryStatus | null;
};

export type ManagedInstallTerminalTask = Extract<
  ManagedInstallTaskState,
  { status: "completed" | "failed" | "cancelled" }
>;

export function isManagedInstallTaskTerminal(
  state: ManagedInstallTaskState,
): state is ManagedInstallTerminalTask {
  return state.status === "completed" || state.status === "failed" || state.status === "cancelled";
}

export function isManagedInstallTerminalRefreshCurrent(
  task: ManagedInstallTerminalTask,
  currentProfileId: string | null,
  libraryUnchanged: boolean,
) {
  return task.profileId === currentProfileId && libraryUnchanged;
}

function isPersistentRecoveryStatus(status: ModInstallSummaryStatus) {
  return status === "committed_cleanup_pending"
    || status === "cleanup_pending"
    || status === "rollback_required"
    || status === "repair_required"
    || status === "unknown";
}

function completedStatusForOperation(operation: ManagedInstallTaskOperation): ModInstallSummaryStatus {
  return operation === "uninstall" ? "not_installed" : "installed";
}

export function shouldFailClosedManagedInstallTerminal(
  task: ManagedInstallTerminalTask,
  refresh: ManagedInstallTerminalRefresh,
) {
  if (!refresh.verified || refresh.status === null) {
    return true;
  }

  if (isPersistentRecoveryStatus(refresh.status)) {
    return false;
  }

  return task.status === "completed" && refresh.status !== completedStatusForOperation(task.operation);
}

export function getManagedInstallTerminalToast(
  task: ManagedInstallTerminalTask,
  refresh: ManagedInstallTerminalRefresh,
  toasts: ModLifecycleCopy["terminalToasts"],
): ModLifecycleToast | null {
  if (task.taskId === null || !refresh.verified || refresh.status === null || isPersistentRecoveryStatus(refresh.status)) {
    return null;
  }

  if (task.status === "completed") {
    if (refresh.status !== completedStatusForOperation(task.operation)) {
      return null;
    }

    const title = task.operation === "uninstall" ? toasts.uninstallCompleted : toasts.installCompleted;
    return { id: task.taskId, title, message: task.modName, tone: "success" };
  }

  if (task.status === "cancelled") {
    return refresh.status === "not_installed"
      ? { id: task.taskId, title: toasts.installCancelled, message: task.modName, tone: "neutral" }
      : null;
  }

  return {
    id: task.taskId,
    title: task.operation === "uninstall" ? toasts.uninstallFailed : toasts.installFailed,
    message: task.message,
    tone: "danger",
  };
}

export function failClosedModInstallSummary(items: ModLibraryItem[], modId: string): ModLibraryItem[] {
  return items.map((item) => {
    if (item.id !== modId) {
      return item;
    }

    return {
      ...item,
      status: "unknown",
      installSummary: {
        status: "unknown",
        managedFileCount: item.installSummary?.managedFileCount ?? 0,
        backupCount: item.installSummary?.backupCount ?? 0,
        recoveryStatus: "unknown",
        issueCount: item.installSummary?.issueCount ?? 0,
        issues: item.installSummary?.issues ?? [],
      },
    };
  });
}
