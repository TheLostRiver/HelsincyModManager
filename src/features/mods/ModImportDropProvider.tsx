import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { getModStorageFreezeReason } from "../settings/modStorageTypes";
import { useModStorageSettings } from "../settings/ModStorageSettingsProvider";
import { ModImportDropOverlay } from "./ModImportDropOverlay";
import { useModLibrarySessionCache } from "./ModLibrarySessionCacheProvider";
import { modImportCopy } from "./modImportCopy";
import { previewDroppedModArchives, startImportModTask } from "./modImportApi";
import { ModImportTaskWatcher, runDropImportPump } from "./modImportDropRunner";
import { dedupeDroppedPaths, MAX_DROPPED_ARCHIVES } from "./modImportDropState";
import {
  activeDropBatches,
  beginDropPreview,
  cancelDropQueued,
  clearDropHistory,
  completeDropPreview,
  confirmDropDraft,
  discardDropDraft,
  dropBatchSummary,
  emptyDropImportSession,
  failedDropBatchPaths,
  failDropPreview,
  finishedDropBatches,
  isDropPreviewCurrent,
  removeDropDraftItem,
  selectDropDraft,
  settleDropBatchItem,
  startDropBatchItem,
  type DropImportQueueItem,
  type DropImportSession,
  type DropListTab,
  type DropPreviewRequest,
} from "./modImportDropSession";
import { getModImportArchiveKeptMessage, getModImportFailedMessage, modImportStartFailureKind } from "./modImportTaskState";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";

// 窗口级宿主持有会话草稿与串行队列，切换页面不会停止任务。
// 未提交草稿可丢弃；批次终态只来自后端，路径不能替代批次/条目/任务身份。
// 关闭整个应用后的队列恢复不属于这份会话状态的能力。
type ModImportDropContextValue = {
  openDropList: () => void;
  libraryRevision: number;
  activeImportCount: number;
};

const ModImportDropContext = createContext<ModImportDropContextValue | null>(null);
const NOTICE_ID = "mod-import.drop.batch";

export function ModImportDropProvider({ children }: { children: ReactNode }) {
  const { pushToast, showTaskNotice, dismissTaskNotice } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(modImportCopy, locale);
  const modStorage = useModStorageSettings();
  const storageWriteFreezeReason = getModStorageFreezeReason(modStorage.writesFrozen, locale);
  const librarySessionCache = useModLibrarySessionCache();

  const [dragActive, setDragActive] = useState(false);
  const [visible, setVisible] = useState(false);
  const [tab, setTab] = useState<DropListTab>("pending");
  const [noticeHidden, setNoticeHidden] = useState(false);
  const [session, setSession] = useState<DropImportSession>(emptyDropImportSession);
  const [libraryRevision, setLibraryRevision] = useState(0);
  const [listenerReady, setListenerReady] = useState(false);
  const sessionRef = useRef(session);
  const identityRef = useRef(0);
  const mountedRef = useRef(false);
  const watcherRef = useRef(new ModImportTaskWatcher());
  const queueRef = useRef<DropImportQueueItem[]>([]);
  const pumpRunningRef = useRef(false);
  const announcedBatchesRef = useRef(new Set<string>());
  const copyRef = useRef(copy);
  const freezeReasonRef = useRef(storageWriteFreezeReason);

  useLayoutEffect(() => {
    copyRef.current = copy;
    freezeReasonRef.current = storageWriteFreezeReason;
  }, [copy, storageWriteFreezeReason]);

  // 只在事件/任务回调中同步提交；React updater 内不执行入队、IPC 或通知副作用。
  // 同一 tick 的第二次确认也能立即看见草稿已被消费，StrictMode 不会双启任务。
  const updateSession = useCallback((update: (current: DropImportSession) => DropImportSession) => {
    const next = update(sessionRef.current);
    sessionRef.current = next;
    setSession(next);
    return next;
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    let disposed = false;
    let unlisten: (() => void) | null = null;
    const watcher = watcherRef.current;
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (!disposed) watcher.handleProgress(event.payload);
    }).then((dispose) => {
      if (disposed) { dispose(); return; }
      unlisten = dispose;
      setListenerReady(true);
    }).catch(() => {
      if (!disposed) setListenerReady(false);
    });
    return () => {
      disposed = true;
      mountedRef.current = false;
      unlisten?.();
    };
  }, []);

  const ensurePumpRunning = useCallback(function ensurePumpRunning() {
    if (pumpRunningRef.current || !mountedRef.current) return;
    pumpRunningRef.current = true;
    void runDropImportPump({
      watcher: watcherRef.current,
      takeNext: () => mountedRef.current ? queueRef.current.shift() ?? null : null,
      startImport: (archivePath) => startImportModTask({ archivePath }),
      onStarted: (item) => {
        updateSession((current) => startDropBatchItem(current, item));
      },
      onSettled: (item, outcome) => {
        if (!mountedRef.current) return;
        // 与会话缓存自身的监听器按 taskId 去重，监听器降级时仍刷新库。
        if (outcome.taskId !== null) {
          librarySessionCache.observeTask({ taskId: outcome.taskId, kind: "mod_import", status: outcome.status });
        }
        updateSession((current) => settleDropBatchItem(current, item, outcome));
        if (outcome.status === "completed") {
          setLibraryRevision((revision) => revision + 1);
          if (outcome.archiveKept !== null) {
            pushToast({
              eventKey: "mod-import.archive-kept." + outcome.taskId,
              taskId: outcome.taskId,
              title: copyRef.current.toasts.archiveKeptTitle,
              message: getModImportArchiveKeptMessage(outcome.archiveKept, copyRef.current),
              tone: "warning",
            });
          }
        }
      },
    }).finally(() => {
      // 不在 takeNext 返回 null 时提前清标志：finally 之前的新入队由此处重新唤醒，
      // 避免旧泵的 finally 清掉新泵标志，或让追加项停在空队列边界。
      pumpRunningRef.current = false;
      if (queueRef.current.length > 0 && mountedRef.current) ensurePumpRunning();
    });
  }, [librarySessionCache, pushToast, updateSession]);

  const handleDroppedPaths = useCallback(async (paths: readonly string[], source: "drop" | "retry" = "drop") => {
    const unique = dedupeDroppedPaths(paths);
    if (unique.length === 0) return;
    const frozen = freezeReasonRef.current;
    if (frozen) {
      pushToast({ eventKey: "mod-import.drop.disabled", title: copyRef.current.drop.title, message: frozen, tone: "warning" });
      return;
    }
    if (source === "drop" && unique.length > MAX_DROPPED_ARCHIVES) {
      pushToast({
        eventKey: "mod-import.drop.too-many",
        title: copyRef.current.drop.title,
        message: copyRef.current.drop.tooMany(unique.length, MAX_DROPPED_ARCHIVES),
        tone: "warning",
      });
      return;
    }
    let next = sessionRef.current;
    let addedCount = 0;
    let duplicateCount = 0;
    const requests: DropPreviewRequest[] = [];
    // 多次拖入可组成较大的批次；重试仍遵守每个预检请求的上限。
    // 同步预留全部分段身份，关闭后迟到的分段不能重新打开草稿。
    for (let offset = 0; offset < unique.length; offset += MAX_DROPPED_ARCHIVES) {
      const preview = beginDropPreview(
        next, unique.slice(offset, offset + MAX_DROPPED_ARCHIVES), "drop-preview-" + ++identityRef.current,
      );
      next = preview.session;
      addedCount += next.draft?.addedCount ?? 0;
      duplicateCount += next.draft?.duplicateCount ?? 0;
      if (preview.request) requests.push(preview.request);
    }
    updateSession(() => next.draft ? { ...next, draft: { ...next.draft, addedCount, duplicateCount } } : next);
    setTab("pending");
    setVisible(true);
    setNoticeHidden(false);
    await Promise.all(requests.map(async (request) => {
      try {
        const previews = await previewDroppedModArchives(request.paths);
        if (!mountedRef.current) return;
        updateSession((current) => completeDropPreview(current, request, previews));
      } catch (error) {
        if (!mountedRef.current || !isDropPreviewCurrent(sessionRef.current, request)) return;
        const kind = modImportStartFailureKind(error);
        updateSession((current) => failDropPreview(current, request, kind));
        pushToast({
          eventKey: "mod-import.drop.preview-failed",
          title: copyRef.current.drop.title,
          message: kind === "preview-limit" ? getModImportFailedMessage(kind, copyRef.current) : copyRef.current.drop.previewFailed,
          tone: "danger",
        });
      }
    }));
  }, [pushToast, updateSession]);

  // Tauri 的拖放事件是窗口级的；预检和导入路径只接受该入口给出的真实路径。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void Promise.resolve().then(() => getCurrentWebview().onDragDropEvent((event) => {
      if (disposed) return;
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") { setDragActive(true); return; }
      if (payload.type === "leave") { setDragActive(false); return; }
      setDragActive(false);
      void handleDroppedPaths(payload.paths);
    })).then((dispose) => {
      if (disposed) { dispose(); return; }
      unlisten = dispose;
    }).catch(() => {
      // 不支持原生拖放时，文件选择器入口仍然可用。
    });
    return () => { disposed = true; unlisten?.(); };
  }, [handleDroppedPaths]);

  const activeBatches = useMemo(() => activeDropBatches(session), [session]);
  const activeImportCount = activeBatches.reduce((count, batch) => {
    const summary = dropBatchSummary(batch);
    return count + summary.queued + summary.running;
  }, 0);

  const openDropList = useCallback(() => {
    const current = sessionRef.current;
    setTab(current.draft ? "pending" : activeDropBatches(current).length > 0 ? "active"
      : finishedDropBatches(current).length > 0 ? "history" : "pending");
    setNoticeHidden(false);
    setVisible(true);
  }, []);

  const openDropHistory = useCallback(() => {
    setTab("history");
    setNoticeHidden(false);
    setVisible(true);
  }, []);

  const hideNotice = useCallback(() => setNoticeHidden(true), []);

  useEffect(() => {
    if (!visible && activeImportCount > 0 && !noticeHidden) {
      showTaskNotice({
        taskId: NOTICE_ID,
        title: copy.drop.openTasks,
        message: copy.drop.backgroundSummary(activeBatches.length, activeImportCount),
        tone: "progress",
        action: { label: copy.drop.reopenList, onClick: openDropList },
        dismiss: { label: copy.drop.hideNotice, onClick: hideNotice },
      });
    } else {
      dismissTaskNotice(NOTICE_ID);
    }
  }, [activeBatches.length, activeImportCount, copy, dismissTaskNotice, hideNotice, noticeHidden, openDropList, showTaskNotice, visible]);

  useEffect(() => () => dismissTaskNotice(NOTICE_ID), [dismissTaskNotice]);

  useEffect(() => {
    const retainedIds = new Set(session.batches.map((batch) => batch.id));
    const announced = new Set([...announcedBatchesRef.current].filter((id) => retainedIds.has(id)));
    let finishedNow = false;
    for (const batch of finishedDropBatches(session)) {
      if (announced.has(batch.id)) continue;
      announced.add(batch.id);
      finishedNow = true;
      const summary = dropBatchSummary(batch);
      pushToast({
        eventKey: "mod-import.drop.finished." + batch.id,
        title: copy.drop.batchTitle(batch.number) + " · " + copy.drop.finishedTitle,
        message: copy.drop.batchResult(summary.succeeded, summary.failed, summary.cancelled, summary.skipped),
        tone: summary.failed > 0 ? "danger" : summary.cancelled > 0 ? "neutral" : "success",
        durationMs: summary.failed > 0 ? 5000 : 3000,
        action: { label: copy.drop.tabHistory, onSelect: openDropHistory },
      });
    }
    announcedBatchesRef.current = announced;
    if (finishedNow && activeBatches.length === 0) {
      setTab((current) => current === "active" ? "history" : current);
    }
  }, [activeBatches.length, copy, openDropHistory, pushToast, session]);

  const closeDropList = useCallback(() => {
    updateSession(discardDropDraft);
    setVisible(false);
  }, [updateSession]);

  const confirm = useCallback(() => {
    if (!listenerReady) return;
    const frozen = freezeReasonRef.current;
    if (frozen) {
      pushToast({ eventKey: "mod-import.drop.disabled", title: copyRef.current.drop.title, message: frozen, tone: "warning" });
      return;
    }
    const { session: next, queued } = confirmDropDraft(
      sessionRef.current, "drop-batch-" + ++identityRef.current, Date.now(),
    );
    if (queued.length === 0) return;
    updateSession(() => next);
    queueRef.current.push(...queued);
    setTab("active");
    setNoticeHidden(false);
    ensurePumpRunning();
  }, [ensurePumpRunning, listenerReady, pushToast, updateSession]);

  const value = useMemo<ModImportDropContextValue>(
    () => ({ openDropList, libraryRevision, activeImportCount }),
    [activeImportCount, libraryRevision, openDropList],
  );

  return (
    <ModImportDropContext.Provider value={value}>
      {children}
      <ModImportDropOverlay
        copy={copy}
        locale={locale}
        dragActive={dragActive}
        visible={visible}
        session={session}
        tab={tab}
        listenerReady={listenerReady}
        onTabChange={setTab}
        onClose={closeDropList}
        onSelectItem={(itemId, selected) => updateSession((current) => selectDropDraft(current, selected, itemId))}
        onSelectAll={(selected) => updateSession((current) => selectDropDraft(current, selected))}
        onRemoveItem={(itemId) => updateSession((current) => removeDropDraftItem(current, itemId))}
        onClearDraft={() => updateSession(discardDropDraft)}
        onConfirm={confirm}
        onCancelQueued={() => {
          queueRef.current = [];
          updateSession(cancelDropQueued);
        }}
        onClearHistory={() => updateSession(clearDropHistory)}
        onRetryBatch={(batchId) => { void handleDroppedPaths(failedDropBatchPaths(sessionRef.current, batchId), "retry"); }}
      />
    </ModImportDropContext.Provider>
  );
}

export function useModImportDrop(): ModImportDropContextValue {
  const context = useContext(ModImportDropContext);
  if (!context) throw new Error("useModImportDrop must be used inside ModImportDropProvider.");
  return context;
}
