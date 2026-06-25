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
