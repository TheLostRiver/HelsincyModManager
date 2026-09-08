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
import { getModStorageFreezeReason } from "../settings/modStorageTypes";
import { useModStorageSettings } from "../settings/ModStorageSettingsProvider";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { ModImportDropOverlay } from "./ModImportDropOverlay";
import { useModLibrarySessionCache } from "./ModLibrarySessionCacheProvider";
import { modImportCopy } from "./modImportCopy";
import { previewDroppedModArchives, startImportModTask } from "./modImportApi";
import { ModImportTaskWatcher, runDropImportPump } from "./modImportDropRunner";
import { getModImportArchiveKeptMessage, getModImportFailedMessage, modImportStartFailureKind } from "./modImportTaskState";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import {
  cancelQueuedDropRows,
  clearFinishedDropRows,
  confirmDropSelection,
  dedupeDroppedPaths,
  dropQueueSummary,
  getDropQueueStatus,
  emptyDropListState,
  markDropRowPhase,
  MAX_DROPPED_ARCHIVES,
  mergeDropRows,
  setAllDropRowsSelected,
  settleDropRow,
  selectedDropRows,
  selectableDropRowCount,
  toggleDropRow,
  type DropListState,
} from "./modImportDropState";

// 拖拽导入的宿主（T22 / #366）。
//
// ## 为什么挂在 RouterOutlet 之上而不是库页里
//
// 首版挂在 `ModLibraryPage` 内部，于是有两个缺陷，而前者掩盖了后者：
//
// 1. 浮层是模态的、导入期间关不掉——拖 30 个包，整个 HMM 就不可用几分钟
// 2. **批量循环是组件里的一个 async 循环，切页组件卸载，循环就死在半路**。已起的后端任务
//    会跑完，队列里剩下的**永远不会起，而且不报错**
//
// 只修 1 会让 2 立刻暴露，所以一起改：状态与循环都搬到这一层。这与 `ExternalStateSessionProvider`
// （#286）、`ModStorageSettingsProvider`（#275）、`InstallConfigTargetProvider`（#354 D4）
// 是同一个形状——`App.tsx` 里那几条注释已经说过 `RouterOutlet` 会卸载页面。
//
// **诚实的边界**：这只解决路由切换。**关掉整个 HMM 仍然会丢掉队列剩余部分**——已起的后端
// 任务会跑完，没起的不会补。要那个程度的健壮得让后端接管批量编排，是另一个量级的活。
//
// ## 拖放在任何页面都接
//
// 拖放事件本来就是**窗口级**的，只在库页接等于让玩家猜哪里能放。清单是覆盖层不是路由跳转，
// 所以在设置页拖一个包进来也不会把他从当前任务里拽走。

type ModImportDropContextValue = {
  /** 打开清单浮层（例如从进度通知点进来）。 */
  openDropList: () => void;
  /** 库内容变更计数。库页订阅它做刷新，不必把回调穿过路由。 */
  libraryRevision: number;
};

const ModImportDropContext = createContext<ModImportDropContextValue | null>(null);

type ModImportDropProviderProps = {
  children: ReactNode;
};

export function ModImportDropProvider({ children }: ModImportDropProviderProps) {
  const { pushToast, showTaskNotice, dismissTaskNotice } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(modImportCopy, locale);
  // #275：存储写入冻结（迁移中 / 待重启）时不能导入。**在开清单之前就挡**——
  // 不挡的话玩家会拖 20 个包、确认、然后眼看着 20 条一个个失败。
  const modStorage = useModStorageSettings();
  const storageWriteFreezeReason = getModStorageFreezeReason(modStorage.writesFrozen, locale);
  const librarySessionCache = useModLibrarySessionCache();

  const [dragActive, setDragActive] = useState(false);
  const [visible, setVisible] = useState(false);
  const [list, setList] = useState<DropListState>(emptyDropListState);
  /*
   * 已提交清单的镜像。确认导入必须在 setState 的 updater **之外**算：
   * updater 要求是纯函数，而 StrictMode 在开发模式下会把它调用两次——正是为了
   * 暴露不纯的 updater。之前入队写在 updater 里，于是同一个包被 push 两次，
   * 泵照队列起了两个导入任务，库里出现两份一模一样的 Mod。
   */
  const listRef = useRef<DropListState>(list);
  useLayoutEffect(() => {
    listRef.current = list;
  }, [list]);
  const [libraryRevision, setLibraryRevision] = useState(0);
  const [listenerReady, setListenerReady] = useState(false);

  const watcherRef = useRef(new ModImportTaskWatcher());
  // 队列与「泵是否在跑」都用 ref：`takeNext` 必须同步，而 state 的读取会落后一拍。
  const queueRef = useRef<string[]>([]);
  const pumpRunningRef = useRef(false);
  const copyRef = useRef(copy);
  // 拖放回调不该因为文案或冻结状态变化就重订阅，所以走 ref 读最新值。
  const freezeReasonRef = useRef(storageWriteFreezeReason);
  useLayoutEffect(() => {
    copyRef.current = copy;
    freezeReasonRef.current = storageWriteFreezeReason;
  }, [copy, storageWriteFreezeReason]);

  // 任务进度订阅。**导入是否算完全靠它**，所以订阅没建起来时不允许确认导入
  // ——否则队列会等一个永远不来的终态，界面卡在「正在导入」。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    const watcher = watcherRef.current;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed) return;
      watcher.handleProgress(event.payload);
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
        setListenerReady(true);
      })
      .catch(() => {
        if (!disposed) setListenerReady(false);
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const ensurePumpRunning = useCallback(() => {
    if (pumpRunningRef.current) return;
    pumpRunningRef.current = true;
    void runDropImportPump({
      watcher: watcherRef.current,
      // 同步取，且「取到 null」与「清标志」在同一 tick——入队方只要事后调一次本函数即可。
      takeNext: () => {
        const next = queueRef.current.shift() ?? null;
        if (next === null) pumpRunningRef.current = false;
        return next;
      },
      startImport: (archivePath) => startImportModTask({ archivePath }),
      onStarted: (archivePath) => {
        setList((current) => ({
          ...current,
          rows: markDropRowPhase(current.rows, archivePath, "running"),
        }));
      },
      onSettled: (archivePath, outcome) => {
        // The queue watcher remains a fallback if the cache's own listener failed.
        // observeTask deduplicates the same terminal identity across both listeners.
        if (outcome.taskId !== null) {
          librarySessionCache.observeTask({ taskId: outcome.taskId, kind: "mod_import", status: outcome.status });
        }
        setList((current) => ({
          ...current,
          rows: settleDropRow(current.rows, archivePath, outcome),
        }));
        if (outcome.status === "completed") {
          // 库页挂着的话，靠这个计数订阅刷新一次：清单现在是长活的，玩家可能一边导一边看库。
          setLibraryRevision((revision) => revision + 1);
          // Cache invalidation is owned by the session-level task observer, including other import entry points.
          if (outcome.archiveKept !== null) {
            pushToast({
              eventKey: `mod-import.archive-kept.${outcome.taskId}`,
              taskId: outcome.taskId,
              title: copyRef.current.toasts.archiveKeptTitle,
              message: getModImportArchiveKeptMessage(outcome.archiveKept, copyRef.current),
              tone: "warning",
            });
          }
        }
      },
    }).finally(() => {
      pumpRunningRef.current = false;
    });
  }, [librarySessionCache, pushToast]);

  const handleDroppedPaths = useCallback(
    async (paths: readonly string[]) => {
      const unique = dedupeDroppedPaths(paths);
      if (unique.length === 0) return;

      const frozen = freezeReasonRef.current;
      if (frozen) {
        pushToast({
          eventKey: "mod-import.drop.disabled",
          title: copyRef.current.drop.title,
          message: frozen,
          tone: "warning",
        });
        return;
      }

      // 超上限**整批拒绝并说清楚数量**，不截断：截断等于悄悄丢掉玩家拖进来的东西。
      if (unique.length > MAX_DROPPED_ARCHIVES) {
        pushToast({
          eventKey: "mod-import.drop.too-many",
          title: copyRef.current.drop.title,
          message: copyRef.current.drop.tooMany(unique.length, MAX_DROPPED_ARCHIVES),
          tone: "warning",
        });
        return;
      }

      setVisible(true);
      setList((current) => ({ ...current, checking: current.checking + unique.length }));
      try {
        const previews = await previewDroppedModArchives(unique);
        setList((current) => ({
          rows: mergeDropRows(current.rows, previews),
          checking: Math.max(0, current.checking - unique.length),
        }));
      } catch (error) {
        setList((current) => ({
          ...current,
          checking: Math.max(0, current.checking - unique.length),
        }));
        pushToast({
          eventKey: "mod-import.drop.preview-failed",
          title: copyRef.current.drop.title,
          message: modImportStartFailureKind(error) === "preview-limit"
            ? getModImportFailedMessage("preview-limit", copyRef.current)
            : copyRef.current.drop.previewFailed,
          tone: "danger",
        });
      }
    },
    [pushToast],
  );

  // Tauri 的拖放事件是**窗口级**的，而且只有它能拿到真实文件路径
  // （浏览器的 DataTransfer 在 webview 里给不出可用路径）。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (disposed) return;
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          setDragActive(true);
          return;
        }
        if (payload.type === "leave") {
          setDragActive(false);
          return;
        }
        setDragActive(false);
        // 导入进行中照样接：新的一批并进同一份清单，队列可追加。
        void handleDroppedPaths(payload.paths);
      })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => {
        // 订阅不上就是没有拖拽功能，其余入口不受影响，不打扰玩家。
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [handleDroppedPaths]);

  const summary = useMemo(() => dropQueueSummary(list.rows), [list.rows]);
  const openDropList = useCallback(() => setVisible(true), []);

  // 浮层关掉之后，进度走既有的任务通知（与单个导入同一套，不另造一份）。
  //
  // Preserve the result entry until rows are explicitly cleared, including failures and pending selections.
  const NOTICE_ID = "mod-import.drop.batch";
  useEffect(() => {
    if (!visible && (list.rows.length > 0 || list.checking > 0)) {
      showTaskNotice({
        taskId: NOTICE_ID,
        title: copy.drop.title,
        message: getDropQueueStatus(summary, copy)
          ?? (list.checking > 0 ? copy.drop.checking(list.checking)
            : copy.drop.selectedSummary(selectedDropRows(list.rows).length, selectableDropRowCount(list.rows))),
        tone: summary.active || list.checking > 0 ? "progress"
          : summary.failed > 0 ? "danger" : summary.succeeded > 0 ? "success" : "neutral",
        action: { label: copy.drop.reopenList, onClick: openDropList },
      });
      return;
    }
    dismissTaskNotice(NOTICE_ID);
  }, [
    dismissTaskNotice,
    copy,
    list.checking,
    list.rows,
    openDropList,
    showTaskNotice,
    summary,
    visible,
  ]);

  const value = useMemo<ModImportDropContextValue>(
    () => ({ openDropList, libraryRevision }),
    [libraryRevision, openDropList],
  );

  return (
    <ModImportDropContext.Provider value={value}>
      {children}
      <ModImportDropOverlay
        copy={copy}
        dragActive={dragActive}
        visible={visible}
        list={list}
        summary={summary}
        listenerReady={listenerReady}
        onClose={() => setVisible(false)}
        onToggleRow={(archivePath) =>
          setList((current) => ({ ...current, rows: toggleDropRow(current.rows, archivePath) }))
        }
        onSelectAll={(selected) =>
          setList((current) => ({
            ...current,
            rows: setAllDropRowsSelected(current.rows, selected),
          }))
        }
        onConfirm={() => {
          if (!listenerReady) return;
          const frozen = freezeReasonRef.current;
          if (frozen) {
            pushToast({ eventKey: "mod-import.drop.disabled", title: copyRef.current.drop.title, message: frozen, tone: "warning" });
            return;
          }
          // 从**同一份快照**做决定：入队的路径与标成 queued 的行必须一致，
          // 所以先算完再一次性落地，不在 updater 里边算边做。
          const current = listRef.current;
          const { rows, queued } = confirmDropSelection(current.rows);
          if (queued.length === 0) return;

          queueRef.current.push(...queued);
          setList({ ...current, rows });
          // 入队之后再唤醒泵。`takeNext` 是同步的，所以「泵刚好跑空」与「这里入队」
          // 不会互相错过。
          ensurePumpRunning();
        }}
        onCancelQueued={() => {
          queueRef.current = [];
          setList((current) => ({ ...current, rows: cancelQueuedDropRows(current.rows) }));
        }}
        onClearFinished={() =>
          setList((current) => ({ ...current, rows: clearFinishedDropRows(current.rows) }))
        }
      />
    </ModImportDropContext.Provider>
  );
}

export function useModImportDrop(): ModImportDropContextValue {
  const context = useContext(ModImportDropContext);
  if (!context) {
    throw new Error("useModImportDrop must be used inside ModImportDropProvider.");
  }
  return context;
}
