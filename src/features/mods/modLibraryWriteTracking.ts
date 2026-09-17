import type { ModLibrarySessionStore, ModLibraryWriteTarget } from "./modLibrarySessionStore.ts";
import type { GetInstallManifestStatusInput, InstallManifestStatusSummary, InstallRecoverySummary, ScanInstallRecoveryInput } from "./modInstallPlanTypes";
import type { TaskProgressEventDto, TaskStartedDto } from "./modImportTypes.ts";

// The application provider owns the store; API starts register before invoking IPC so a
// route change or a terminal event arriving before the start reply cannot lose the writer.
let store: ModLibrarySessionStore | null = null;
const listeners = new Set<(event: { payload: TaskProgressEventDto }) => void>();

export function attachModLibraryWriteTracking(value: ModLibrarySessionStore) {
  store = value;
  return () => { if (store === value) store = null; };
}

export function publishModLibraryTaskProgress(payload: TaskProgressEventDto) {
  store?.observeTask(payload);
  for (const listener of listeners) listener({ payload });
}

export async function listenModLibraryTaskProgress(listener: (event: { payload: TaskProgressEventDto }) => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export async function trackModLibraryTaskStart(
  target: { gameId: string; profileId: string; modId: string },
  start: () => Promise<TaskStartedDto>,
): Promise<TaskStartedDto> {
  const owner = store;
  const token = owner?.beginWrite(target);
  try {
    const task = await start();
    if (token !== undefined) owner?.bindWriteTask(token, task);
    return task;
  } catch (error) {
    if (token !== undefined) owner?.finishWrite(token);
    throw error;
  }
}

/** Batch start/retry currently resolves only after the whole attempt has stopped. Its
 * journal/result uses a different runtime, so it must not be polled in desktop TaskManager. */
export async function trackModLibraryBatchWrite<T extends { task: TaskStartedDto }>(start: () => Promise<T>, target?: ModLibraryWriteTarget): Promise<T> {
  const owner = store;
  const token = owner?.beginWrite(target);
  try {
    const result = await start();
    if (token !== undefined) owner?.bindWriteTask(token, result.task);
    return result;
  } finally {
    if (token !== undefined) owner?.finishWrite(token);
  }
}

/** Catalog changes survive route unmounts and failed replies. Invalidate before releasing
 * occupancy so an outer batch still performs only one terminal catalog query. */
export async function trackModLibraryCatalogWrite<T>(write: () => Promise<T>): Promise<T> {
  const owner = store;
  const token = owner?.beginWrite();
  try {
    return await write();
  } finally {
    owner?.invalidateAllPages();
    if (token !== undefined) owner?.finishWrite(token);
  }
}

export async function trackModLibraryIntegrityScan(input: ScanInstallRecoveryInput, scan: () => Promise<InstallRecoverySummary[]>): Promise<InstallRecoverySummary[]> {
  const finish = store?.beginIntegrityScan(input.gameId, input.profileId, input.modIds);
  try {
    const summaries = await scan();
    finish?.(summaries);
    return summaries;
  } catch (error) {
    finish?.(null);
    throw error;
  }
}

/** The legacy gameId form also performs an integrity scan. Keep findings from detail
 * and plugin workflows; the manifest-only form must never clear integrity evidence. */
export async function trackModLibraryManifestScan(input: GetInstallManifestStatusInput, query: () => Promise<InstallManifestStatusSummary[]>): Promise<InstallManifestStatusSummary[]> {
  if (input.gameId === undefined) return query();
  const finish = store?.beginIntegrityScan(input.gameId, input.profileId, input.modIds);
  try {
    const summaries = await query();
    finish?.(summaries.map((summary) => ({ ...summary, status: summary.status === "installed" ? "completed" : summary.status,
      adoptedFileCount: summary.adoptedFileCount ?? 0, issueCount: 0, issues: [] })));
    return summaries;
  } catch (error) {
    finish?.(null);
    throw error;
  }
}
