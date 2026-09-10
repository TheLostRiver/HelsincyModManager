import type { CategoryItem } from "./modCategoryApi";
import type { TaskProgressEventDto } from "./modImportTypes";
import type { ModLibraryPage } from "./modLibraryTypes";
import {
  EMPTY_MOD_LIBRARY_SESSION_CACHE,
  invalidateAllCachedLibraryPages,
  invalidateCachedLibraryPage,
  readCachedCategories,
  readCachedLibraryPage,
  writeCachedCategories,
  writeCachedLibraryPage,
} from "./modLibrarySessionCache.ts";

export function createModLibrarySessionStore() {
  let cache = EMPTY_MOD_LIBRARY_SESSION_CACHE;
  let generation = 0;
  let available = false;
  const subscribers = new Set<() => void>();
  const activeTasks = new Set<string>();
  const observedTasks = new Map<string, boolean>();

  const invalidateAllPages = () => {
    cache = invalidateAllCachedLibraryPages(cache);
    generation += 1;
    for (const notify of subscribers) notify();
  };

  return {
    getGeneration: () => generation,
    subscribe: (notify: () => void) => {
      subscribers.add(notify);
      return () => { subscribers.delete(notify); };
    },
    readPage: (profileKey: string, queryKey: string) =>
      available && activeTasks.size === 0 ? readCachedLibraryPage(cache, profileKey, queryKey) : null,
    writePage: (profileKey: string, queryKey: string, page: ModLibraryPage, expectedGeneration: number) => {
      if (!available || activeTasks.size > 0 || expectedGeneration !== generation) return;
      cache = writeCachedLibraryPage(cache, profileKey, queryKey, page);
    },
    // Eviction after a query failure must not trigger an endless automatic retry.
    invalidatePage: (profileKey: string, queryKey: string) => {
      cache = invalidateCachedLibraryPage(cache, profileKey, queryKey);
    },
    invalidateAllPages,
    readCategories: () => readCachedCategories(cache),
    writeCategories: (categories: readonly CategoryItem[], expectedGeneration: number) => {
      if (expectedGeneration !== generation) return;
      cache = writeCachedCategories(cache, categories);
    },
    setAvailable: (next: boolean) => {
      if (available === next) return;
      available = next;
      invalidateAllPages();
    },
    observeTask: (event: Pick<TaskProgressEventDto, "taskId" | "kind" | "status"> & Partial<Pick<TaskProgressEventDto, "phase">>) => {
      if (!["mod_import", "install", "external_mod_adopt"].includes(event.kind)) return;
      // Source discovery shares the import task kind but does not write library facts.
      if (event.phase?.startsWith("external_import.scan.")) return;
      const terminal = ["completed", "failed", "cancelled"].includes(event.status);
      const previous = observedTasks.get(event.taskId);
      if (previous === true || previous === terminal) return;
      observedTasks.set(event.taskId, terminal);
      if (terminal) activeTasks.delete(event.taskId);
      else activeTasks.add(event.taskId);
      // Retain active identities; bound only the terminal deduplication history.
      if (observedTasks.size > 512) {
        for (const [taskId, finished] of observedTasks) {
          if (finished) observedTasks.delete(taskId);
          if (observedTasks.size <= 512) break;
        }
      }
      invalidateAllPages();
    },
  };
}

export type ModLibrarySessionStore = ReturnType<typeof createModLibrarySessionStore>;
