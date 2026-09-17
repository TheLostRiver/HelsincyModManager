import type { ModLibrarySessionStore } from "./modLibrarySessionStore.ts";
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
export async function trackModLibraryBatchWrite<T extends { task: TaskStartedDto }>(start: () => Promise<T>): Promise<T> {
  const owner = store;
  const token = owner?.beginWrite();
  try {
    const result = await start();
    if (token !== undefined) owner?.bindWriteTask(token, result.task);
    return result;
  } finally {
    if (token !== undefined) owner?.finishWrite(token);
  }
}
