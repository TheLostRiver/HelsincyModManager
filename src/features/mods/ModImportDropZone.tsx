import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { CheckCircle2, FileArchive, LoaderCircle, Upload, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modImportCopy, type ModImportCopy } from "./modImportCopy";
import { previewDroppedModArchives, startImportModTask } from "./modImportApi";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { ModImportTaskWatcher, runDropImportBatch } from "./modImportDropRunner";
import {
  canStartDropImport,
  dedupeDroppedPaths,
  dropImportRunSummary,
  dropRowsFromPreviews,
  dropSelectAllState,
  getDropRowNote,
  isDropRowSelectable,
  MAX_DROPPED_ARCHIVES,
  setAllDropRowsSelected,
  toggleDropRow,
  type DropImportRun,
  type DropListState,
  type DropRow,
} from "./modImportDropState";
import "./ModImportDropZone.css";

// 拖拽导入（T22 / #366）。
//
// **拖进来不等于导入。** 落点是一份可逐条勾选的待导入清单，玩家确认之后才真的导入
// ——批量拖十几个文件时，直接开跑是不可撤销的，而清单让「哪些能导、哪些读不了」
// 在动手之前就说清楚。
//
// 判定全部来自后端 `preview_dropped_mod_archives`（与真正导入同一条嗅探链路），
// 前端**不**自己实现一份「什么算压缩包」——两处判定迟早会漂。

type ModImportDropZoneProps = {
  /** 有值 = 现在不能导入（存储冻结等），拖进来只提示原因，不开清单。 */
  disabledReason?: string | null;
  onImported: () => Promise<void> | void;
};

/** 二进制单位，与备份中心、诊断页的口径一致。 */
function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GiB`;
}

function runStatusText(run: DropImportRun, copy: ModImportCopy): string {
  const summary = dropImportRunSummary(run);
  if (!summary.done) {
    return copy.drop.running(summary.finished + 1, summary.total);
  }
  if (summary.failed === 0) return copy.drop.doneAllSucceeded(summary.succeeded);
  if (summary.succeeded === 0) return copy.drop.doneAllFailed(summary.failed);
  return copy.drop.donePartial(summary.succeeded, summary.failed);
}

export function ModImportDropZone({ disabledReason, onImported }: ModImportDropZoneProps) {
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(modImportCopy, locale);

  const [dragActive, setDragActive] = useState(false);
  const [listState, setListState] = useState<DropListState>({ status: "idle" });
  const [run, setRun] = useState<DropImportRun | null>(null);
  const [listenerReady, setListenerReady] = useState(false);

  // 起任务 / 认领终态 / 串行推进都在 modImportDropRunner 里，那里能用假依赖逐条断言；
  // 这个组件只负责订阅事件、渲染清单。
  const watcherRef = useRef(new ModImportTaskWatcher());
  const abortRef = useRef(false);
  // 每次新拖拽 +1。预检是异步的，而玩家可以在它回来之前就关掉浮层或者再拖一次；
  // 不认这个号的话，落地的旧结果会把已经关掉的清单重新弹出来。
  const dropGenerationRef = useRef(0);
  const selectAllRef = useRef<HTMLInputElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  // 拖放回调要读最新值，但它不该因此重订阅。
  const dropsBlockedRef = useRef(false);
  const disabledReasonRef = useRef(disabledReason);
  disabledReasonRef.current = disabledReason;

  // 任务进度订阅。**导入是否算完全靠它**，所以订阅没建起来时不允许确认导入
  // ——否则批量循环会等一个永远不来的终态，界面卡在「正在导入」。
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

  const handleDroppedPaths = useCallback(
    async (paths: readonly string[]) => {
      const unique = dedupeDroppedPaths(paths);
      if (unique.length === 0) return;

      // 超上限**整批拒绝并说清楚数量**，不截断：截断等于悄悄丢掉玩家拖进来的东西。
      if (unique.length > MAX_DROPPED_ARCHIVES) {
        pushToast({
          eventKey: "mod-import.drop.too-many",
          title: copy.drop.title,
          message: copy.drop.tooMany(unique.length, MAX_DROPPED_ARCHIVES),
          tone: "warning",
        });
        return;
      }

      const reason = disabledReasonRef.current;
      if (reason) {
        pushToast({
          eventKey: "mod-import.drop.disabled",
          title: copy.drop.title,
          message: reason,
          tone: "warning",
        });
        return;
      }

      dropGenerationRef.current += 1;
      const generation = dropGenerationRef.current;
      setRun(null);
      setListState({ status: "checking", total: unique.length });
      try {
        const previews = await previewDroppedModArchives(unique);
        if (generation !== dropGenerationRef.current) return;
        setListState({ status: "ready", rows: dropRowsFromPreviews(previews) });
      } catch {
        if (generation !== dropGenerationRef.current) return;
        setListState({ status: "idle" });
        pushToast({
          eventKey: "mod-import.drop.preview-failed",
          title: copy.drop.title,
          message: copy.drop.previewFailed,
          tone: "danger",
        });
      }
    },
    [copy, pushToast],
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
          if (!dropsBlockedRef.current) setDragActive(true);
          return;
        }
        if (payload.type === "leave") {
          setDragActive(false);
          return;
        }
        setDragActive(false);
        // 清单开着的时候不收新的拖拽：它是模态的，而且直接替换会把玩家刚勾好的
        // 选择悄悄丢掉。高亮同样不显示——不给「能放」的暗示，才不会白拖一次。
        if (dropsBlockedRef.current) return;
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

  const rows: DropRow[] =
    listState.status === "ready" || listState.status === "importing" ? listState.rows : [];
  const selectAll = dropSelectAllState(rows);
  const blockedCount = rows.filter((row) => row.status === "blocked").length;
  const selectedCount = rows.filter((row) => row.status === "importable" && row.selected).length;
  const importableCount = rows.length - blockedCount;
  const importing = listState.status === "importing";
  const runFinished = run !== null && dropImportRunSummary(run).done;
  // **跑完之后仍处于 importing 态**（清单要留着给玩家看结果），所以「忙」不能等于
  // importing——否则关闭按钮点了没反应。
  const busy = importing && !runFinished;
  dropsBlockedRef.current = listState.status !== "idle";

  useEffect(() => {
    if (selectAllRef.current) selectAllRef.current.indeterminate = selectAll === "some";
  }, [selectAll]);

  // 浮层打开时把焦点收进来：否则 Tab 会走到底下被遮住的控件上，键盘用户
  // 完全不知道自己在哪儿。
  useEffect(() => {
    if (listState.status !== "idle") panelRef.current?.focus();
  }, [listState.status]);

  function closeList() {
    if (busy) return;
    // 作废还在飞的那次预检：否则它落地时会把刚关掉的清单重新弹出来。
    dropGenerationRef.current += 1;
    setListState({ status: "idle" });
    setRun(null);
  }

  // Esc 关闭：与库里其他浮层一致。导入进行中不响应（closeList 自己挡住）。
  useEffect(() => {
    if (listState.status === "idle") return undefined;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeList();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  async function confirmImport() {
    if (listState.status !== "ready" || !canStartDropImport(rows) || !listenerReady) return;

    abortRef.current = false;
    setListState({ status: "importing", rows });
    await runDropImportBatch(rows, {
      watcher: watcherRef.current,
      startImport: (archivePath) => startImportModTask({ archivePath }),
      onRunChange: setRun,
      // 「停止」只保证不再往下起；正在跑的那个由后端任务机制管到底。
      shouldStop: () => abortRef.current,
    });

    // 整批跑完只刷新一次库：每个都刷会在批量导入时反复重排列表。
    try {
      await onImported();
    } catch {
      pushToast({
        eventKey: "mod-import.drop.refresh-failed",
        title: copy.toasts.refreshFailedTitle,
        message: copy.toasts.refreshFailedMessage,
        tone: "warning",
      });
    }
  }

  return (
    <>
      {dragActive && listState.status === "idle" ? (
        <div className="mod-import-drop__hint" role="presentation">
          <div className="mod-import-drop__hint-card">
            <Upload size={28} strokeWidth={2.2} aria-hidden="true" />
            <span>{copy.drop.hint}</span>
          </div>
        </div>
      ) : null}

      {listState.status !== "idle" ? (
        <div className="mod-import-drop__backdrop" role="presentation" onClick={closeList}>
          <div
            ref={panelRef}
            tabIndex={-1}
            className="mod-import-drop__panel"
            role="dialog"
            aria-modal="true"
            aria-label={copy.drop.title}
            onClick={(event) => event.stopPropagation()}
          >
            <h2 className="mod-import-drop__title">{copy.drop.title}</h2>

            {listState.status === "checking" ? (
              <p className="mod-import-drop__status" role="status">
                <LoaderCircle className="mod-import-drop__spinner" size={16} aria-hidden="true" />
                {copy.drop.checking(listState.total)}
              </p>
            ) : (
              <>
                <div className="mod-import-drop__summary">
                  <label className="mod-import-drop__select-all">
                    <input
                      ref={selectAllRef}
                      type="checkbox"
                      checked={selectAll === "all"}
                      disabled={importing || importableCount === 0}
                      onChange={(event) =>
                        setListState({
                          status: "ready",
                          rows: setAllDropRowsSelected(rows, event.target.checked),
                        })
                      }
                    />
                    <span>{copy.drop.selectAll}</span>
                  </label>
                  <span className="mod-import-drop__counts">
                    {copy.drop.selectedSummary(selectedCount, importableCount)}
                    {blockedCount > 0 ? ` · ${copy.drop.blockedSummary(blockedCount)}` : ""}
                  </span>
                </div>

                <ul className="mod-import-drop__rows">
                  {rows.map((row) => {
                    const note = getDropRowNote(row, copy);
                    const outcome = run?.results[row.archivePath];
                    const active = run?.currentPath === row.archivePath;
                    return (
                      <li
                        key={row.archivePath}
                        className="mod-import-drop__row"
                        data-status={row.status}
                      >
                        <label className="mod-import-drop__row-main">
                          <input
                            type="checkbox"
                            checked={row.selected}
                            // 读不了的行不是「默认不选」，是**不能选**：链路物理上读不了它。
                            disabled={!isDropRowSelectable(row) || importing}
                            onChange={() =>
                              setListState({
                                status: "ready",
                                rows: toggleDropRow(rows, row.archivePath),
                              })
                            }
                          />
                          <FileArchive size={14} aria-hidden="true" />
                          <span className="mod-import-drop__file-name" title={row.archivePath}>
                            {row.fileName}
                          </span>
                        </label>
                        {row.sizeBytes !== null ? (
                          <span className="mod-import-drop__size">
                            {formatBytes(row.sizeBytes)}
                          </span>
                        ) : null}
                        {note ? (
                          <span className="mod-import-drop__row-note">{note}</span>
                        ) : null}
                        {active ? (
                          <LoaderCircle
                            className="mod-import-drop__spinner"
                            size={14}
                            aria-hidden="true"
                          />
                        ) : null}
                        {outcome === "succeeded" ? (
                          <CheckCircle2
                            className="mod-import-drop__ok"
                            size={14}
                            aria-hidden="true"
                          />
                        ) : null}
                        {outcome === "failed" ? (
                          <XCircle className="mod-import-drop__bad" size={14} aria-hidden="true" />
                        ) : null}
                      </li>
                    );
                  })}
                </ul>

                {run !== null ? (
                  <p className="mod-import-drop__status" role="status" aria-live="polite">
                    {runStatusText(run, copy)}
                  </p>
                ) : importableCount === 0 ? (
                  <p className="mod-import-drop__status" role="status">
                    {copy.drop.nothingImportable}
                  </p>
                ) : !listenerReady ? (
                  <p className="mod-import-drop__status" role="status">
                    {copy.status.listenerFailedHint}
                  </p>
                ) : null}

                <div className="mod-import-drop__actions">
                  {busy ? (
                    <button
                      type="button"
                      className="mod-import-drop__button"
                      onClick={() => {
                        abortRef.current = true;
                      }}
                    >
                      {copy.drop.cancel}
                    </button>
                  ) : (
                    <button type="button" className="mod-import-drop__button" onClick={closeList}>
                      {runFinished ? copy.drop.close : copy.drop.cancel}
                    </button>
                  )}
                  {!importing ? (
                    <button
                      type="button"
                      className="mod-import-drop__button is-primary"
                      disabled={!canStartDropImport(rows) || !listenerReady}
                      onClick={() => void confirmImport()}
                    >
                      {copy.drop.confirm}
                    </button>
                  ) : null}
                </div>
              </>
            )}
          </div>
        </div>
      ) : null}
    </>
  );
}
