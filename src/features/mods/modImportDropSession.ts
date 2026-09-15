import {
  dedupeDroppedPaths,
  dropQueueSummary,
  dropRowsFromPreviews,
  isDropRowSelectable,
  setAllDropRowsSelected,
  type DroppedArchivePreview,
  type DropRow,
} from "./modImportDropState.ts";
import type { ModImportFailedMessageKind, ModImportTerminalState } from "./modImportTaskState";

export type DropListTab = "pending" | "active" | "history";

export type DropDraftItem = {
  id: string;
  requestId: string;
  archivePath: string;
  /** null 只表示这条仍在预检，不能提交。 */
  row: DropRow | null;
};

export type DropDraft = {
  id: string;
  items: DropDraftItem[];
  addedCount: number;
  duplicateCount: number;
};

export type DropBatchItem = { id: string; row: DropRow };

export type DropImportBatch = {
  id: string;
  number: number;
  createdAt: number;
  items: DropBatchItem[];
};

export type DropImportSession = {
  draft: DropDraft | null;
  batches: DropImportBatch[];
  nextBatchNumber: number;
};

export type DropPreviewRequest = { draftId: string; requestId: string; paths: string[] };
export type DropImportQueueItem = { batchId: string; itemId: string; archivePath: string };

export const MAX_DROP_HISTORY_BATCHES = 50;
export const emptyDropImportSession: DropImportSession = { draft: null, batches: [], nextBatchNumber: 1 };

export function dropBatchSummary(batch: DropImportBatch) {
  return dropQueueSummary(batch.items.map((item) => item.row));
}

export function activeDropBatches(session: DropImportSession): DropImportBatch[] {
  return session.batches.filter((batch) => dropBatchSummary(batch).active);
}

export function finishedDropBatches(session: DropImportSession): DropImportBatch[] {
  return session.batches.filter((batch) => !dropBatchSummary(batch).active);
}

function retainDropHistory(session: DropImportSession): DropImportSession {
  const finished = finishedDropBatches(session);
  if (finished.length <= MAX_DROP_HISTORY_BATCHES) return session;
  const expired = new Set(finished.slice(0, -MAX_DROP_HISTORY_BATCHES).map((batch) => batch.id));
  return { ...session, batches: session.batches.filter((batch) => !expired.has(batch.id)) };
}

/** 先预留行和请求身份，避免并发预检返回顺序改变拖入顺序或重复入队。 */
export function beginDropPreview(
  session: DropImportSession,
  paths: readonly string[],
  requestId: string,
): { session: DropImportSession; request: DropPreviewRequest | null } {
  const existing = new Set(session.draft?.items.map((item) => item.archivePath));
  for (const batch of session.batches) {
    for (const { row } of batch.items) {
      if (row.phase === "queued" || row.phase === "running") existing.add(row.archivePath);
    }
  }
  const unique = dedupeDroppedPaths(paths);
  const accepted = unique.filter((path) => !existing.has(path));
  const draftId = session.draft?.id ?? requestId;
  const items = accepted.map((archivePath, index): DropDraftItem => ({
    id: requestId + ":" + index,
    requestId,
    archivePath,
    row: null,
  }));
  return {
    session: {
      ...session,
      draft: {
        id: draftId,
        items: [...(session.draft?.items ?? []), ...items],
        addedCount: accepted.length,
        duplicateCount: unique.length - accepted.length,
      },
    },
    request: accepted.length === 0 ? null : { draftId, requestId, paths: accepted },
  };
}

export function isDropPreviewCurrent(session: DropImportSession, request: DropPreviewRequest): boolean {
  return session.draft?.id === request.draftId
    && session.draft.items.some((item) => item.requestId === request.requestId && item.row === null);
}

export function completeDropPreview(
  session: DropImportSession,
  request: DropPreviewRequest,
  previews: readonly DroppedArchivePreview[],
): DropImportSession {
  if (!isDropPreviewCurrent(session, request) || !session.draft) return session;
  const byPath = new Map(dropRowsFromPreviews(previews).map((row) => [row.archivePath, row]));
  return {
    ...session,
    draft: {
      ...session.draft,
      items: session.draft.items.map((item) => {
        if (item.requestId !== request.requestId || item.row !== null) return item;
        // 缺少对应结果时不能推断“可以导入”，也不接受响应中额外的路径。
        const row = byPath.get(item.archivePath) ?? failedPreviewRow(item.archivePath, "retry-hint");
        return { ...item, row };
      }),
    },
  };
}

function failedPreviewRow(archivePath: string, messageKind: ModImportFailedMessageKind): DropRow {
  const [row] = dropRowsFromPreviews([{
    archivePath,
    fileName: archivePath.split(/[\\/]/).pop() || archivePath,
    sizeBytes: null,
    errorCode: "mod_import_prepare_failed",
    warningCode: null,
  }]);
  return { ...row, messageKind };
}

export function failDropPreview(
  session: DropImportSession,
  request: DropPreviewRequest,
  messageKind: ModImportFailedMessageKind,
): DropImportSession {
  if (!isDropPreviewCurrent(session, request) || !session.draft) return session;
  return {
    ...session,
    draft: {
      ...session.draft,
      items: session.draft.items.map((item) => item.requestId === request.requestId && item.row === null
        ? { ...item, row: failedPreviewRow(item.archivePath, messageKind) } : item),
    },
  };
}

export function dropDraftChecking(draft: DropDraft | null): number {
  return draft?.items.filter((item) => item.row === null).length ?? 0;
}

export function dropDraftRows(draft: DropDraft | null): DropRow[] {
  return draft?.items.flatMap((item) => item.row === null ? [] : [item.row]) ?? [];
}

export function selectDropDraft(session: DropImportSession, selected: boolean, itemId?: string): DropImportSession {
  if (!session.draft) return session;
  return {
    ...session,
    draft: {
      ...session.draft,
      items: session.draft.items.map((item) => item.row && isDropRowSelectable(item.row)
        && (itemId === undefined || item.id === itemId)
        ? { ...item, row: setAllDropRowsSelected([item.row], selected)[0] } : item),
    },
  };
}

export function removeDropDraftItem(session: DropImportSession, itemId: string): DropImportSession {
  if (!session.draft) return session;
  const items = session.draft.items.filter((item) => item.id !== itemId);
  return { ...session, draft: items.length === 0 ? null : { ...session.draft, items } };
}

export function discardDropDraft(session: DropImportSession): DropImportSession {
  return session.draft ? { ...session, draft: null } : session;
}

/** 确认快照只生成一次；调用者同步保存返回的 session 后再执行入队副作用。 */
export function confirmDropDraft(
  session: DropImportSession,
  batchId: string,
  createdAt: number,
): { session: DropImportSession; queued: DropImportQueueItem[] } {
  const draft = session.draft;
  if (!draft || dropDraftChecking(draft) > 0) return { session, queued: [] };
  const items = draft.items.flatMap(({ id, row }): DropBatchItem[] => row === null ? [] : [{
    id,
    row: { ...row, selected: false, phase: isDropRowSelectable(row) && row.selected ? "queued" : "skipped" },
  }]);
  const queued = items.filter((item) => item.row.phase === "queued").map((item) => ({
    batchId, itemId: item.id, archivePath: item.row.archivePath,
  }));
  if (queued.length === 0) return { session, queued };
  return {
    session: {
      draft: null,
      batches: [...session.batches, { id: batchId, number: session.nextBatchNumber, createdAt, items }],
      nextBatchNumber: session.nextBatchNumber + 1,
    },
    queued,
  };
}

function updateBatchItem(
  session: DropImportSession,
  target: DropImportQueueItem,
  update: (row: DropRow) => DropRow,
): DropImportSession {
  return retainDropHistory({
    ...session,
    batches: session.batches.map((batch) => batch.id !== target.batchId ? batch : {
      ...batch,
      items: batch.items.map((item) => item.id !== target.itemId ? item : { ...item, row: update(item.row) }),
    }),
  });
}

export function startDropBatchItem(session: DropImportSession, target: DropImportQueueItem): DropImportSession {
  return updateBatchItem(session, target, (row) => row.phase === "queued" ? { ...row, phase: "running" } : row);
}

export function settleDropBatchItem(
  session: DropImportSession,
  target: DropImportQueueItem,
  outcome: ModImportTerminalState,
): DropImportSession {
  return updateBatchItem(session, target, (row) => row.phase !== "running" ? row : {
    ...row,
    phase: outcome.status === "completed" ? "succeeded" : outcome.status === "cancelled" ? "cancelled" : "failed",
    outcome,
  });
}

export function cancelDropQueued(session: DropImportSession): DropImportSession {
  return retainDropHistory({
    ...session,
    batches: session.batches.map((batch) => ({
      ...batch,
      items: batch.items.map((item) => item.row.phase === "queued"
        ? { ...item, row: { ...item.row, phase: "cancelled", selected: false } } : item),
    })),
  });
}

export function clearDropHistory(session: DropImportSession): DropImportSession {
  return { ...session, batches: activeDropBatches(session) };
}

export function failedDropBatchPaths(session: DropImportSession, batchId: string): string[] {
  return session.batches.find((batch) => batch.id === batchId)?.items
    .filter((item) => item.row.phase === "failed").map((item) => item.row.archivePath) ?? [];
}
