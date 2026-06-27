import type { InstallManifestStatusSummary, InstallRecoverySummary } from "./modInstallPlanTypes";
import type { ModInstallSummaryStatus, ModLibraryItem } from "./modLibraryTypes";

type ResolveLoadedModLibraryItemsInput = {
  backendItems: ModLibraryItem[] | null;
  fallbackItems: ModLibraryItem[];
};

export function resolveLoadedModLibraryItems({
  backendItems,
  fallbackItems,
}: ResolveLoadedModLibraryItemsInput): ModLibraryItem[] {
  return backendItems ?? fallbackItems;
}

export function applyInstallManifestStatusSummaries(
  items: ModLibraryItem[],
  summaries: InstallManifestStatusSummary[],
): ModLibraryItem[] {
  if (items.length === 0 || summaries.length === 0) {
    return items;
  }

  const summaryByModId = new Map(summaries.map((summary) => [summary.modId, summary]));

  return items.map((item) => {
    const summary = summaryByModId.get(item.id);
    if (!summary) {
      return item;
    }

    const status = item.status === "disabled" || item.status === "conflict" ? item.status : summary.status;

    return {
      ...item,
      status,
      installSummary: {
        status: summary.status,
        managedFileCount: summary.managedFileCount,
        backupCount: summary.backupCount,
      },
    };
  });
}

function recoveryStatusToInstallStatus(status: InstallRecoverySummary["status"]): ModInstallSummaryStatus {
  return status === "completed" ? "installed" : status;
}

export function applyInstallRecoverySummaries(
  items: ModLibraryItem[],
  summaries: InstallRecoverySummary[],
): ModLibraryItem[] {
  if (items.length === 0 || summaries.length === 0) {
    return items;
  }

  const summaryByModId = new Map(summaries.map((summary) => [summary.modId, summary]));

  return items.map((item) => {
    const summary = summaryByModId.get(item.id);
    if (!summary) {
      return item;
    }

    const installStatus = recoveryStatusToInstallStatus(summary.status);
    const safetyStatus =
      installStatus === "rollback_required" || installStatus === "repair_required" || installStatus === "unknown"
        ? installStatus
        : item.status === "disabled" || item.status === "conflict"
          ? item.status
          : installStatus;

    return {
      ...item,
      status: safetyStatus,
      installSummary: {
        status: installStatus,
        managedFileCount: summary.managedFileCount,
        backupCount: summary.backupCount,
        recoveryStatus: summary.status,
        issueCount: summary.issueCount,
        issues: summary.issues,
      },
    };
  });
}

export function applyInstallRecoveryUnavailable(items: ModLibraryItem[]): ModLibraryItem[] {
  return items.map((item) => {
    const summary = item.installSummary;
    if (!summary || summary.status === "not_installed") {
      return item;
    }

    return {
      ...item,
      status: item.status === "disabled" || item.status === "conflict" ? item.status : "unknown",
      installSummary: {
        ...summary,
        status: "unknown",
        recoveryStatus: "unknown",
        issueCount: summary.issueCount ?? 0,
        issues: summary.issues ?? [],
      },
    };
  });
}
