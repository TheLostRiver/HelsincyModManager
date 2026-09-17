import type { CategoryItem } from "./modCategoryApi";
import type { TaskProgressEventDto } from "./modImportTypes";
import type { ModLibraryPage, ModLibraryProfileContext, QueryModLibraryInput } from "./modLibraryTypes";
import type { InstallRecoverySummary } from "./modInstallPlanTypes";
import type { ModInstallationStateEvent, ModInstallationStateUpdate } from "./modInstallationStateTypes";
import { createModInstallationStateCache } from "./modInstallationStateCache.ts";
import {
  EMPTY_MOD_LIBRARY_SESSION_CACHE,
  invalidateAllCachedLibraryPages,
  invalidateCachedLibraryPage,
  readCachedCategories,
  readCachedLibraryPage,
  writeCachedCategories,
  writeCachedLibraryPage,
} from "./modLibrarySessionCache.ts";

export type ModLibraryWriteTarget = { gameId: string; profileId: string } & ({ modId: string } | { modIds: string[] });

export function createModLibrarySessionStore() {
  let cache = EMPTY_MOD_LIBRARY_SESSION_CACHE;
  let displayCache = EMPTY_MOD_LIBRARY_SESSION_CACHE;
  let generation = 0;
  let available = false;
  const subscribers = new Set<() => void>();
  const displaySubscribers = new Set<() => void>();
  const installationStates = createModInstallationStateCache();
  const activeTasks = new Set<string>();
  const observedTasks = new Map<string, boolean>();
  const pendingWrites = new Set<number>();
  let writeSequence = 0;
  const dirtyTargets = new Map<string, Set<string>>();
  const statusReads = new Map<string, { generation: number; verified: boolean }>();
  let integritySequence = 0;
  const isWriting = () => activeTasks.size > 0 || pendingWrites.size > 0;
  const scopeKey = (gameId: string, profileId: string) => JSON.stringify([gameId, profileId]);

  const invalidateStatusReads = () => {
    statusReads.clear();
    generation += 1;
    for (const notify of subscribers) notify();
  };
  const notifyDisplay = () => { for (const notify of displaySubscribers) notify(); };
  const invalidateAllPages = () => {
    cache = invalidateAllCachedLibraryPages(cache);
    invalidateStatusReads();
  };
  const projectPage = (page: ModLibraryPage, context: ModLibraryProfileContext | null) => context
    ? installationStates.project(page, context.gameId, context.profileId) : page;
  const projectSlot = (page: ModLibraryPage | null, profileKey: string) => {
    if (!page || !profileKey.startsWith("profile:") || !profileKey.includes("\u0000")) return page;
    const [gameId, profileId] = profileKey.slice(8).split("\u0000");
    return projectPage(page, { gameId, profileId });
  };
  const cachedModIds = (gameId: string, profileId: string) => {
    const profileKey = `profile:${gameId}\u0000${profileId}`;
    return [...new Set([...cache.pages, ...displayCache.pages]
      .filter((entry) => entry.profileKey === profileKey).flatMap((entry) => entry.page.items.map((item) => item.id)))];
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
    // An import can finish while an installation still owns the outer boundary.
    // Remember its catalog change now, even if this event does not schedule a query.
    if (event.kind === "mod_import") cache = invalidateAllCachedLibraryPages(cache);
    if (wasWriting !== isWriting() || (!wasWriting && terminal)) invalidateStatusReads();
  };

  const finishWrite = (token: number) => {
    if (!pendingWrites.delete(token)) return;
    if (!isWriting()) invalidateStatusReads();
  };

  return {
    getGeneration: () => generation,
    getDisplayVersion: installationStates.getVersion,
    subscribeDisplay: (notify: () => void) => {
      displaySubscribers.add(notify);
      return () => { displaySubscribers.delete(notify); };
    },
    projectPage,
    isWriting,
    waitForWrites: () => new Promise<void>((resolve) => {
      if (!isWriting()) { resolve(); return; }
      const check = () => {
        if (!isWriting()) { subscribers.delete(check); resolve(); }
      };
      subscribers.add(check);
    }),
    activeTaskIds: () => [...activeTasks],
    beginWrite: (target?: ModLibraryWriteTarget) => {
      const wasWriting = isWriting();
      const token = ++writeSequence;
      pendingWrites.add(token);
      if (target) {
        const key = scopeKey(target.gameId, target.profileId);
        const ids = dirtyTargets.get(key) ?? new Set<string>();
        for (const id of "modId" in target ? [target.modId] : target.modIds) ids.add(id);
        dirtyTargets.set(key, ids);
      }
      if (!wasWriting) invalidateStatusReads();
      return token;
    },
    bindWriteTask: (token: number, task: Pick<TaskProgressEventDto, "taskId" | "kind" | "status">) => {
      observeTask(task);
      finishWrite(token);
    },
    finishWrite,
    pendingStatusModIds: (gameId: string, profileId: string) => [...(dirtyTargets.get(scopeKey(gameId, profileId)) ?? [])],
    cachedModIds,
    acceptInstallationStates: (update: ModInstallationStateUpdate, expectedGeneration: number) => {
      if (isWriting() || expectedGeneration !== generation) return false;
      const accepted = installationStates.apply(update, "query");
      notifyDisplay();
      return accepted;
    },
    observeInstallationState: (event: ModInstallationStateEvent) => {
      if (!event.taskId) return;
      const accepted = installationStates.apply(event, "event");
      notifyDisplay();
      if (!accepted && !isWriting()) invalidateStatusReads();
    },
    finishStatusRead: (gameId: string, profileId: string, expectedGeneration: number, modIds: string[], verified: boolean) => {
      if (isWriting() || expectedGeneration !== generation) return;
      const key = scopeKey(gameId, profileId);
      statusReads.set(key, { generation, verified });
      if (verified) {
        const pending = dirtyTargets.get(key);
        for (const id of modIds) pending?.delete(id);
        if (pending?.size === 0) dirtyTargets.delete(key);
      } else installationStates.unavailable(gameId, profileId);
      notifyDisplay();
    },
    readStatusSnapshot: (gameId: string, profileId: string) => {
      const read = statusReads.get(scopeKey(gameId, profileId));
      return !isWriting() && read?.generation === generation
        ? { ...read, summaries: installationStates.summaries(gameId, profileId) } : null;
    },
    beginIntegrityScan: (gameId: string, profileId: string, modIds: string[]) => {
      const startedGeneration = generation;
      const sequence = ++integritySequence;
      const startedWhileWriting = isWriting();
      return (summaries: InstallRecoverySummary[] | null) => {
        if (startedWhileWriting || isWriting() || startedGeneration !== generation) return;
        const valid = summaries !== null && new Set(summaries.map((item) => item.modId)).size === summaries.length
          && summaries.every((item) => item.profileId === profileId && (modIds.length === 0 || modIds.includes(item.modId)))
          && modIds.every((id) => summaries.some((item) => item.modId === id));
        installationStates.recordIntegrity(gameId, profileId, modIds, sequence, valid ? summaries : null);
        notifyDisplay();
      };
    },
    subscribe: (notify: () => void) => {
      subscribers.add(notify);
      return () => { subscribers.delete(notify); };
    },
    readPage: (profileKey: string, queryKey: string) =>
      available && !isWriting() ? projectSlot(readCachedLibraryPage(cache, profileKey, queryKey), profileKey) : null,
    readDisplayPage: (profileKey: string, queryKey: string) => projectSlot(readCachedLibraryPage(displayCache, profileKey, queryKey), profileKey),
    readCatalogPage: (input: QueryModLibraryInput) => {
      if (!available || input.filter.kind === "status") return null;
      const context = input.profileContext;
      const profileKey = context ? `profile:${context.gameId}\u0000${context.profileId}` : "profile:none";
      return readCachedLibraryPage(cache, profileKey, JSON.stringify(input));
    },
    writePage: (profileKey: string, queryKey: string, page: ModLibraryPage, expectedGeneration: number) => {
      if (!available || isWriting() || expectedGeneration !== generation) return;
      cache = writeCachedLibraryPage(cache, profileKey, queryKey, page);
      displayCache = writeCachedLibraryPage(displayCache, profileKey, queryKey, page);
    },
    // Eviction after a query failure must not trigger an endless automatic retry.
    invalidatePage: (profileKey: string, queryKey: string) => {
      cache = invalidateCachedLibraryPage(cache, profileKey, queryKey);
      statusReads.clear();
      if (profileKey.startsWith("profile:") && profileKey.includes("\u0000")) {
        const [gameId, profileId] = profileKey.slice(8).split("\u0000");
        installationStates.unavailable(gameId, profileId);
      }
    },
    invalidateAllPages,
    readCategories: () => readCachedCategories(cache),
    writeCategories: (categories: readonly CategoryItem[], expectedGeneration: number) => {
      if (expectedGeneration !== generation) return;
      const previous = readCachedCategories(cache);
      const changed = previous === null ? cache.pages.length > 0 : (previous.length !== categories.length || categories.some((category, index) => {
        const before = previous[index];
        return before.id !== category.id || before.name !== category.name || before.color !== category.color
          || before.sortOrder !== category.sortOrder || before.modCount !== category.modCount;
      }));
      cache = writeCachedCategories(cache, categories);
      // Category edits happen outside the Mod route; its next category read must also
      // invalidate cached card labels, filter membership and counts when facts changed.
      if (changed) invalidateAllPages();
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
