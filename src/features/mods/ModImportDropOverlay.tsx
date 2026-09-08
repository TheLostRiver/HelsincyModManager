import { CheckCircle2, Clock, FileArchive, LoaderCircle, Upload, XCircle } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ModImportCopy } from "./modImportCopy";
import {
  canStartDropImport,
  dropSelectAllState,
  getDropRowNote,
  isDropRowSelectable,
  selectableDropRowCount,
  selectedDropRows,
  type DropListState,
  type DropQueueSummary,
  type DropRow,
} from "./modImportDropState";
import "./ModImportDropOverlay.css";

// 待导入清单浮层（T22 / #366）。纯展示 + 回调，状态与队列都在 ModImportDropProvider。
//
// **它随时可关。** 首版做成了模态、导入期间关不掉，等于拖一批包就把整个 HMM 锁住几分钟。
// 现在关掉只是收起视图，队列在后台继续，进度走既有的任务通知，点通知能重新打开。

type ModImportDropOverlayProps = {
  copy: ModImportCopy;
  dragActive: boolean;
  visible: boolean;
  list: DropListState;
  summary: DropQueueSummary;
  listenerReady: boolean;
  onClose: () => void;
  onToggleRow: (archivePath: string) => void;
  onSelectAll: (selected: boolean) => void;
  onConfirm: () => void;
  onCancelQueued: () => void;
  onClearFinished: () => void;
};

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GiB`;
}

function RowPhaseMark({ row, copy }: { row: DropRow; copy: ModImportCopy }) {
  switch (row.phase) {
    case "queued":
      return (
        <span className="mod-import-drop__phase" title={copy.drop.phaseQueued}>
          <Clock size={14} aria-hidden="true" />
          {copy.drop.phaseQueued}
        </span>
      );
    case "running":
      return (
        <span className="mod-import-drop__phase" title={copy.drop.phaseRunning}>
          <LoaderCircle className="mod-import-drop__spinner" size={14} aria-hidden="true" />
          {copy.drop.phaseRunning}
        </span>
      );
    case "succeeded":
      return <CheckCircle2 className="mod-import-drop__ok" size={16} aria-hidden="true" />;
    case "failed":
      return <XCircle className="mod-import-drop__bad" size={16} aria-hidden="true" />;
    default:
      return null;
  }
}

function statusLine(summary: DropQueueSummary, copy: ModImportCopy): string | null {
  if (summary.running > 0 || summary.queued > 0) {
    return copy.drop.running(summary.succeeded + summary.failed + 1, summary.submitted);
  }
  if (summary.submitted === 0) return null;
  if (summary.failed === 0) return copy.drop.doneAllSucceeded(summary.succeeded);
  if (summary.succeeded === 0) return copy.drop.doneAllFailed(summary.failed);
  return copy.drop.donePartial(summary.succeeded, summary.failed);
}

export function ModImportDropOverlay({
  copy,
  dragActive,
  visible,
  list,
  summary,
  listenerReady,
  onClose,
  onToggleRow,
  onSelectAll,
  onConfirm,
  onCancelQueued,
  onClearFinished,
}: ModImportDropOverlayProps) {
  const selectAllRef = useRef<HTMLInputElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);

  const rows = list.rows;
  const selectAll = dropSelectAllState(rows);
  const selectableCount = selectableDropRowCount(rows);
  const selectedCount = selectedDropRows(rows).length;
  const blockedCount = rows.filter((row) => row.status === "blocked").length;

  useEffect(() => {
    if (selectAllRef.current) selectAllRef.current.indeterminate = selectAll === "some";
  }, [selectAll]);

  // 浮层打开时把焦点收进来：否则 Tab 会走到底下被遮住的控件上。
  useEffect(() => {
    if (visible) panelRef.current?.focus();
  }, [visible]);

  // Esc 关闭。**导入中照样能关**——关闭只是收起视图，队列不受影响。
  useEffect(() => {
    if (!visible) return undefined;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, visible]);

  const status = statusLine(summary, copy);
  const hasFinished = summary.succeeded + summary.failed > 0;

  return (
    <>
      {dragActive && !visible ? (
        <div className="mod-import-drop__hint" role="presentation">
          <div className="mod-import-drop__hint-card">
            <Upload size={28} strokeWidth={2.2} aria-hidden="true" />
            <span>{copy.drop.hint}</span>
          </div>
        </div>
      ) : null}

      {visible ? (
        <div className="mod-import-drop__backdrop" role="presentation" onClick={onClose}>
          <div
            ref={panelRef}
            tabIndex={-1}
            className="mod-import-drop__panel"
            role="dialog"
            aria-label={copy.drop.title}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mod-import-drop__header">
              <h2 className="mod-import-drop__title">{copy.drop.title}</h2>
              {list.checking > 0 ? (
                <span className="mod-import-drop__checking">
                  <LoaderCircle className="mod-import-drop__spinner" size={14} aria-hidden="true" />
                  {copy.drop.checking(list.checking)}
                </span>
              ) : null}
            </div>

            {rows.length > 0 ? (
              <>
                <div className="mod-import-drop__summary">
                  <label className="mod-import-drop__select-all">
                    <input
                      ref={selectAllRef}
                      type="checkbox"
                      checked={selectAll === "all"}
                      disabled={selectableCount === 0}
                      onChange={(event) => onSelectAll(event.target.checked)}
                    />
                    <span>{copy.drop.selectAll}</span>
                  </label>
                  <span className="mod-import-drop__counts">
                    {copy.drop.selectedSummary(selectedCount, selectableCount)}
                    {blockedCount > 0 ? ` · ${copy.drop.blockedSummary(blockedCount)}` : ""}
                  </span>
                </div>

                <ul className="mod-import-drop__rows">
                  {rows.map((row) => {
                    const note = getDropRowNote(row, copy);
                    const selectable = isDropRowSelectable(row);
                    return (
                      <li
                        key={row.archivePath}
                        className="mod-import-drop__row"
                        data-status={row.status}
                        data-phase={row.phase}
                      >
                        {/* 第一行只放定长信息；提示语独占第二行。
                            挤在同一行会把整句中文截成省略号——读不到原因等于没有原因。 */}
                        <div className="mod-import-drop__row-head">
                          <label className="mod-import-drop__row-main">
                            <input
                              type="checkbox"
                              checked={row.selected}
                              // 读不了的行不是「默认不选」，是**不能选**；已提交的行不再归玩家管。
                              disabled={!selectable}
                              onChange={() => onToggleRow(row.archivePath)}
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
                          <RowPhaseMark row={row} copy={copy} />
                        </div>
                        {note ? (
                          <p className="mod-import-drop__row-note">{note}</p>
                        ) : null}
                      </li>
                    );
                  })}
                </ul>
              </>
            ) : list.checking === 0 ? (
              <p className="mod-import-drop__status">{copy.drop.emptyList}</p>
            ) : null}

            {status ? (
              <p className="mod-import-drop__status" role="status" aria-live="polite">
                {status}
              </p>
            ) : rows.length > 0 && selectableCount === 0 && summary.submitted === 0 ? (
              <p className="mod-import-drop__status">{copy.drop.nothingImportable}</p>
            ) : !listenerReady ? (
              <p className="mod-import-drop__status">{copy.status.listenerFailedHint}</p>
            ) : null}

            <div className="mod-import-drop__actions">
              {hasFinished ? (
                <button
                  type="button"
                  className="mod-import-drop__button is-quiet"
                  onClick={onClearFinished}
                >
                  {copy.drop.clearFinished}
                </button>
              ) : null}
              {summary.queued > 0 ? (
                <button type="button" className="mod-import-drop__button" onClick={onCancelQueued}>
                  {copy.drop.stopQueued}
                </button>
              ) : null}
              <button type="button" className="mod-import-drop__button" onClick={onClose}>
                {summary.active ? copy.drop.closeKeepRunning : copy.drop.close}
              </button>
              <button
                type="button"
                className="mod-import-drop__button is-primary"
                disabled={!canStartDropImport(rows) || !listenerReady}
                onClick={onConfirm}
              >
                {copy.drop.confirm}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
