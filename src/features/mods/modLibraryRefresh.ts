import type { ModLibraryItem } from "./modLibraryTypes";
import type { ModDetailDialogTab } from "./ModDetailDialog";

export type ModLibraryLoadMode = "initial" | "refresh";

type LoadModLibraryItemsForModeInput = {
  mode: ModLibraryLoadMode;
  fallbackItems: ModLibraryItem[];
  getModLibrary: () => Promise<ModLibraryItem[]>;
  refreshInstallManifestStatuses: (items: ModLibraryItem[]) => Promise<ModLibraryItem[]>;
};

export type ModLibraryLoadResult =
  | { status: "loaded"; items: ModLibraryItem[] }
  | { status: "fallback"; items: ModLibraryItem[] }
  | { status: "unavailable"; items: null };

export function snapshotModLibraryItem(item: ModLibraryItem): ModLibraryItem {
  return {
    ...item,
    categoryLabels: item.categoryLabels.map((category) => ({ ...category })),
  };
}

export function createDetailDialogState(
  modId: string,
  libraryItems: ModLibraryItem[],
  initialTab: ModDetailDialogTab = "details",
) {
  const fallbackItem = libraryItems.find((item) => item.id === modId) ?? null;
  return {
    modId,
    initialTab,
    fallbackItem: fallbackItem ? snapshotModLibraryItem(fallbackItem) : null,
  };
}

export function preserveItemsOnRefreshFailure(currentItems: ModLibraryItem[], loadedItems: ModLibraryItem[] | null) {
  return loadedItems ?? currentItems;
}

export async function loadModLibraryItemsForMode({
  mode,
  fallbackItems,
  getModLibrary,
  refreshInstallManifestStatuses,
}: LoadModLibraryItemsForModeInput): Promise<ModLibraryLoadResult> {
  try {
    const backendItems = await getModLibrary();
    const resolvedItems = backendItems ?? fallbackItems;
    const items = await refreshInstallManifestStatuses(resolvedItems);
    return { status: "loaded", items };
  } catch {
    if (mode === "initial") {
      return {
        status: "fallback",
        items: fallbackItems,
      };
    }

    return { status: "unavailable", items: null };
  }
}
