import type { InstallManifestStatusSummary } from "./modInstallPlanTypes";
import type { ModLibraryItem } from "./modLibraryTypes";

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
