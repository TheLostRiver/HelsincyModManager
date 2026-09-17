import type { CategoryItem } from "./modCategoryApi";
import type { TaskProgressEventDto } from "./modImportTypes";
import type { ModLibraryPage } from "./modLibraryTypes";
import type { InstallRecoverySummary } from "./modInstallPlanTypes";
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
  let displayCache = EMPTY_MOD_LIBRARY_SESSION_CACHE;
  let generation = 0;
  let available = false;
  const subscribers = new Set<() => void>();
  const activeTasks = new Set<string>();
  const observedTasks = new Map<string, boolean>();
  const pendingWrites = new Set<number>();
  let writeSequence = 0;
  const dirtyTargets = new Map<string, Set<string>>();
  const statusSnapshots = new Map<string, { generation: number; summaries: InstallRecoverySummary[]; verified: boolean }>();
  const isWriting = () => activeTasks.size > 0 || pendingWrites.size > 0;
  const scopeKey = (gameId: string, profileId: string) => JSON.stringify([gameId, profileId]);

  const invalidateAllPages = () => {
    cache = invalidateAllCachedLibraryPages(cache);
    statusSnapshots.clear();
    generation += 1;
    for (const notify of subscribers) notify();
  };

  const observeTask = (event: Pick<TaskProgressEventDto, "taskId" | "kind" | "status"> & Partial<Pick<TaskProgressEventDto, "phase">>) => {
    if (!["mod_import", "install", "external_mod_adopt"].includes(event.kind)) return;
    if (event.phase?.startsWith("external_import.scan.")) return;
    const terminal = ["completed", "failed", "cancelled"].includes(event.status);
    const previous = observedTasks.get(event.taskId);
    if (previous === true || previous === terminal) return;
    const wasWriting = isWriting();
    observedTasks.set(event.taskId, terminal);
    if (terminal) activeTasks.delete(event.taskId);
    else activeTasks.add(event.taskId);
    if (observedTasks.size > 512) {
      for (const [taskId, finished] of observedTasks) {
        if (finished) observedTasks.delete(taskId);
        if (observedTasks.size <= 512) break;
      }
    }
    // A locally registered start already invalidated the generation. Only the outer write
    // boundary schedules work; duplicate events and overlapping writers cannot fan out reads.
    if (wasWriting !== isWriting() || (!wasWriting && terminal)) invalidateAllPages();
  };

  const finishWrite = (token: number) => {
    if (!pendingWrites.delete(token)) return;
    if (!isWriting()) invalidateAllPages();
  };

  return {
    getGeneration: () => generation,
    isWriting,
    waitForWrites: () => new Promise<void>((resolve) => {
      if (!isWriting()) { resolve(); return; }
      const check = () => {
        if (!isWriting()) { subscribers.delete(check); resolve(); }
      };
      subscribers.add(check);
    }),
    activeTaskIds: () => [...activeTasks],
    beginWrite: (target?: { gameId: string; profileId: string; modId: string }) => {
      const wasWriting = isWriting();
      const token = ++writeSequence;
      pendingWrites.add(token);
      if (target) {
        const key = scopeKey(target.gameId, target.profileId);
        const ids = dirtyTargets.get(key) ?? new Set<string>();
        ids.add(target.modId);
        dirtyTargets.set(key, ids);
      }
      if (!wasWriting) invalidateAllPages();
      return token;
    },
    bindWriteTask: (token: number, task: Pick<TaskProgressEventDto, "taskId" | "kind" | "status">) => {
      observeTask(task);
      finishWrite(token);
    },
    finishWrite,
    pendingStatusModIds: (gameId: string, profileId: string) => [...(dirtyTargets.get(scopeKey(gameId, profileId)) ?? [])],
    writeStatusSnapshot: (gameId: string, profileId: string, expectedGeneration: number, summaries: InstallRecoverySummary[], verified: boolean) => {
      if (isWriting() || expectedGeneration !== generation) return;
      const key = scopeKey(gameId, profileId);
      const previous = statusSnapshots.get(key);
      const byModId = new Map(previous?.verified && previous.generation === generation
        ? previous.summaries.map((summary) => [summary.modId, summary]) : []);
      for (const summary of summaries) byModId.set(summary.modId, summary);
      statusSnapshots.set(key, { generation, summaries: verified ? [...byModId.values()] : [], verified });
      if (verified) {
        const pending = dirtyTargets.get(key);
        for (const summary of summaries) pending?.delete(summary.modId);
        if (pending?.size === 0) dirtyTargets.delete(key);
      }
    },
    readStatusSnapshot: (gameId: string, profileId: string) => {
      const snapshot = statusSnapshots.get(scopeKey(gameId, profileId));
      return !isWriting() && snapshot?.generation === generation ? snapshot : null;
    },
    subscribe: (notify: () => void) => {
      subscribers.add(notify);
      return () => { subscribers.delete(notify); };
    },
    readPage: (profileKey: string, queryKey: string) =>
      available && !isWriting() ? readCachedLibraryPage(cache, profileKey, queryKey) : null,
    readDisplayPage: (profileKey: string, queryKey: string) => readCachedLibraryPage(displayCache, profileKey, queryKey),
    writePage: (profileKey: string, queryKey: string, page: ModLibraryPage, expectedGeneration: number) => {
      if (!available || isWriting() || expectedGeneration !== generation) return;
      cache = writeCachedLibraryPage(cache, profileKey, queryKey, page);
      displayCache = writeCachedLibraryPage(displayCache, profileKey, queryKey, page);
    },
    // Eviction after a query failure must not trigger an endless automatic retry.
    invalidatePage: (profileKey: string, queryKey: string) => {
      cache = invalidateCachedLibraryPage(cache, profileKey, queryKey);
      statusSnapshots.clear();
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
    observeTask,
  };
}

export type ModLibrarySessionStore = ReturnType<typeof createModLibrarySessionStore>;
