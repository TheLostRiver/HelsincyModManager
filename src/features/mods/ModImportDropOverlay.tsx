import { LoaderCircle, Upload } from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { FeedbackPortal } from "../../shared/feedback/FeedbackProvider";
import { ModalSurface } from "../../shared/feedback/ModalSurface";
import type { Locale } from "../../shared/i18n";
import type { ModImportCopy } from "./modImportCopy";
import { canStartDropImport, dropSelectAllState, selectedDropRows, selectableDropRowCount } from "./modImportDropState";
import {
  activeDropBatches,
  dropBatchSummary,
  dropDraftChecking,
  dropDraftRows,
  finishedDropBatches,
  MAX_DROP_HISTORY_BATCHES,
  type DropImportSession,
  type DropListTab,
} from "./modImportDropSession";
import { ModImportDropBatch } from "./ModImportDropBatch";
import { ModImportDropRow } from "./ModImportDropRow";
import "./ModImportDropOverlay.css";

type ModImportDropOverlayProps = {
  copy: ModImportCopy;
  locale: Locale;
  dragActive: boolean;
  visible: boolean;
  session: DropImportSession;
  tab: DropListTab;
  listenerReady: boolean;
  onTabChange: (tab: DropListTab) => void;
  onClose: () => void;
  onSelectItem: (itemId: string, selected: boolean) => void;
  onSelectAll: (selected: boolean) => void;
  onRemoveItem: (itemId: string) => void;
  onClearDraft: () => void;
  onConfirm: () => void;
  onCancelQueued: () => void;
  onClearHistory: () => void;
  onRetryBatch: (batchId: string) => void;
};

export function ModImportDropOverlay({
  copy, locale, dragActive, visible, session, tab, listenerReady,
  onTabChange, onClose, onSelectItem, onSelectAll, onRemoveItem, onClearDraft,
  onConfirm, onCancelQueued, onClearHistory, onRetryBatch,
}: ModImportDropOverlayProps) {
  const id = useId();
  const selectAllRef = useRef<HTMLInputElement | null>(null);
  // 业务草稿立即丢弃，离场动画仍展示关闭瞬间的内容，避免先闪空再消失。
  const [closingView, setClosingView] = useState<{ session: DropImportSession; tab: DropListTab } | null>(null);
  const view = visible ? { session, tab } : closingView ?? { session, tab };
  const draft = view.session.draft;
  const rows = dropDraftRows(draft);
  const checking = dropDraftChecking(draft);
  const active = activeDropBatches(view.session);
  const history = finishedDropBatches(view.session);
  const queued = active.reduce((count, batch) => count + dropBatchSummary(batch).queued, 0);
  const remaining = active.reduce((count, batch) => {
    const summary = dropBatchSummary(batch);
    return count + summary.queued + summary.running;
  }, 0);
  const selectAll = dropSelectAllState(rows);
  const selectedCount = selectedDropRows(rows).length;
  const selectableCount = selectableDropRowCount(rows);
  const blockedCount = rows.filter((row) => row.status === "blocked").length;
  const tabs: { id: DropListTab; label: string; count: number }[] = [
    { id: "pending", label: copy.drop.tabPending, count: draft?.items.length ?? 0 },
    { id: "active", label: copy.drop.tabActive, count: active.length },
    { id: "history", label: copy.drop.tabHistory, count: history.length },
  ];

  useEffect(() => {
    if (selectAllRef.current) selectAllRef.current.indeterminate = selectAll === "some";
  }, [selectAll, view.tab, visible]);

  const close = () => {
    setClosingView({ session, tab });
    onClose();
  };
  const closeLabel = draft ? copy.drop.cancelDraft : active.length > 0 ? copy.drop.closeKeepRunning : copy.drop.close;

  const moveTab = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let next: number;
    if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    else return;
    event.preventDefault();
    onTabChange(tabs[next].id);
    event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  };

  return (
    <>
      {dragActive && !visible ? (
        <FeedbackPortal>
          <div className="mod-import-drop__hint" role="presentation">
            <div className="mod-import-drop__hint-card"><Upload size={28} aria-hidden="true" /><span>{copy.drop.hint}</span></div>
          </div>
        </FeedbackPortal>
      ) : null}
      <ModalSurface
        kind="dialog"
        open={visible}
        title={copy.drop.title}
        icon={<Upload size={20} />}
        panelClassName="mod-import-drop__panel"
        closeLabel={closeLabel}
        onClose={close}
        closeOnBackdrop={false}
        footer={(
          <div className="mod-import-drop__footer">
            {draft ? <p className="mod-import-drop__discard-hint">{copy.drop.discardHint}</p> : null}
            <div className="mod-import-drop__actions">
              {view.tab === "pending" && draft ? (
                <button type="button" className="mod-import-drop__button is-quiet" onClick={onClearDraft}>{copy.drop.clearDraft}</button>
              ) : null}
              {view.tab === "history" ? (
                <button type="button" className="mod-import-drop__button is-quiet" disabled={history.length === 0} onClick={onClearHistory}>{copy.drop.clearFinished}</button>
              ) : null}
              {view.tab === "active" && queued > 0 ? (
                <button type="button" className="mod-import-drop__button is-quiet" onClick={onCancelQueued}>{copy.drop.stopQueued}</button>
              ) : null}
              <button type="button" className="mod-import-drop__button" onClick={close}>{closeLabel}</button>
              {view.tab === "pending" ? (
                <button
                  type="button"
                  className="mod-import-drop__button is-primary"
                  disabled={!canStartDropImport(rows) || checking > 0 || !listenerReady}
                  onClick={onConfirm}
                >{active.length > 0 ? copy.drop.appendCount(selectedCount) : copy.drop.confirmCount(selectedCount)}</button>
              ) : null}
            </div>
          </div>
        )}
      >
        <div className="mod-import-drop__tabs" role="tablist" aria-label={copy.drop.tabsLabel}>
          {tabs.map((item, index) => (
            <button
              key={item.id}
              id={id + "-" + item.id}
              type="button"
              role="tab"
              aria-selected={view.tab === item.id}
              aria-controls={id + "-panel"}
              tabIndex={view.tab === item.id ? 0 : -1}
              className="mod-import-drop__tab"
              onClick={() => onTabChange(item.id)}
              onKeyDown={(event) => moveTab(event, index)}
            >{item.label}<span>{item.count}</span></button>
          ))}
        </div>
        <div className="mod-import-drop__view" id={id + "-panel"} role="tabpanel" aria-labelledby={id + "-" + view.tab}>
          {view.tab === "pending" ? (
            <>
              {active.length > 0 ? (
                <button type="button" className="mod-import-drop__background" onClick={() => onTabChange("active")}>
                  <LoaderCircle className="mod-import-drop__spinner" size={14} aria-hidden="true" />
                  {copy.drop.backgroundSummary(active.length, remaining)}
                </button>
              ) : null}
              {draft ? <p className="mod-import-drop__status" role="status">{copy.drop.addedSummary(draft.addedCount, draft.duplicateCount)}</p> : null}
              {draft && draft.items.length > 0 ? (
                <>
                  <div className="mod-import-drop__summary">
                    <label className="mod-import-drop__select-all">
                      <input ref={selectAllRef} type="checkbox" checked={selectAll === "all"} disabled={selectableCount === 0 || checking > 0} onChange={(event) => onSelectAll(event.target.checked)} />
                      <span>{copy.drop.selectAll}</span>
                    </label>
                    <span className="mod-import-drop__counts">{copy.drop.selectedSummary(selectedCount, selectableCount)}{blockedCount > 0 ? " · " + copy.drop.blockedSummary(blockedCount) : ""}</span>
                  </div>
                  <ul className="mod-import-drop__rows mod-import-drop__scroll">
                    {draft.items.map((item) => (
                      <ModImportDropRow key={item.id} archivePath={item.archivePath} row={item.row} copy={copy}
                        onSelect={(selected) => onSelectItem(item.id, selected)} onRemove={() => onRemoveItem(item.id)} />
                    ))}
                  </ul>
                </>
              ) : <p className="mod-import-drop__empty">{copy.drop.emptyList}</p>}
              {checking > 0 ? <p className="mod-import-drop__status" role="status"><LoaderCircle className="mod-import-drop__spinner" size={14} aria-hidden="true" />{copy.drop.checking(checking)}</p> : null}
              {!listenerReady ? <p className="mod-import-drop__status" role="status">{copy.status.listenerFailedHint}</p> : null}
              {checking === 0 && rows.length > 0 && selectableCount === 0 ? <p className="mod-import-drop__status">{copy.drop.nothingImportable}</p> : null}
            </>
          ) : (
            <>
              {view.tab === "history" ? <p className="mod-import-drop__status">{copy.drop.historyHint(MAX_DROP_HISTORY_BATCHES)}</p> : null}
              <div className="mod-import-drop__batches mod-import-drop__scroll">
                {(view.tab === "active" ? active : [...history].reverse()).map((batch, index) => (
                  <ModImportDropBatch key={batch.id} batch={batch} copy={copy} locale={locale}
                    initiallyExpanded={index === 0} onRetry={() => onRetryBatch(batch.id)} />
                ))}
                {(view.tab === "active" ? active : history).length === 0 ? (
                  <p className="mod-import-drop__empty">{view.tab === "active" ? copy.drop.emptyActive : copy.drop.emptyHistory}</p>
                ) : null}
              </div>
            </>
          )}
        </div>
      </ModalSurface>
    </>
  );
}
