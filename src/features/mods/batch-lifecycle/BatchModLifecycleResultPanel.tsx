import { AlertTriangle, CheckCircle2, LoaderCircle, RefreshCw, X } from "lucide-react";
import { useId, useRef } from "react";
import { useModalFocusTrap } from "../../../shared/feedback/useModalFocusTrap";
import { resolveCopy, useI18n } from "../../../shared/i18n";
import {
  batchModLifecycleCopy,
  getBatchAttemptStatusLabel,
  getBatchErrorLabel,
  getBatchItemStatusLabel,
  getBatchOperationLabel,
  getBatchReasonCodeLabel,
} from "./batchModLifecycleCopy.ts";
import type { BatchModLifecycleWorkflowState } from "./batchModLifecycleWorkflow";
import "./BatchModLifecyclePanel.css";

export type BatchModLifecycleResultPanelProps = {
  workflowState: BatchModLifecycleWorkflowState;
  onRetry: () => void;
  onLoadMore: () => void;
  onClose: () => void;
};

function statusTone(status: string): string {
  if (status === "completed") {
    return "success";
  }
  if (status === "completed_with_errors") {
    return "warning";
  }
  if (status === "cancelled") {
    return "neutral";
  }
  return "danger";
}

export function BatchModLifecycleResultPanel({
  workflowState,
  onRetry,
  onLoadMore,
  onClose,
}: BatchModLifecycleResultPanelProps) {
  const { locale } = useI18n();
  const bCopy = resolveCopy(batchModLifecycleCopy, locale);
  const panelCopy = bCopy.resultPanel;
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement | null>(null);
  useModalFocusTrap({
    active: true,
    containerRef: panelRef,
    closeOnEscape: true,
    onRequestClose: onClose,
  });

  if (workflowState.status !== "result") {
    return null;
  }
  const { result, operation, batchId } = workflowState;
  const summary = result.summary;
  const retryAvailableByStatus =
    result.status === "completed_with_errors"
    || result.status === "failed"
    || result.status === "recovery_required";
  const tone = statusTone(result.status);
  const canLoadMore = result.nextCursor !== null;

  return (
    <div className="batch-panel__backdrop" role="presentation">
      <div
        ref={panelRef}
        className="batch-panel batch-panel--result"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <header className="batch-panel__header">
          <h2 id={titleId}>{panelCopy.resultTitle(getBatchOperationLabel(operation, bCopy.operations))}</h2>
          <button
            type="button"
            className="batch-panel__close"
            aria-label={panelCopy.closeAria}
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>

        <div className="batch-panel__body">
          <div className={`batch-panel__attempt batch-panel__attempt--${tone}`} role="status">
            <CheckCircle2 size={16} aria-hidden="true" />
            {getBatchAttemptStatusLabel(result.status, bCopy.attemptStatus)}
            <span className="batch-panel__attempt-id">{panelCopy.batchIdLabel(batchId)}</span>
          </div>

          <div className="batch-panel__summary-counts" aria-label={panelCopy.summaryAria}>
            <span>{panelCopy.succeededCount(summary.succeededCount)}</span>
            <span>{panelCopy.failedCount(summary.failedCount)}</span>
            <span>{panelCopy.blockedCount(summary.blockedCount)}</span>
            <span>{panelCopy.skippedCount(summary.skippedCount)}</span>
            <span>{panelCopy.cancelledCount(summary.cancelledCount)}</span>
            {summary.recoveryRequiredCount > 0 && (
              <span>{panelCopy.recoveryRequiredCount(summary.recoveryRequiredCount)}</span>
            )}
          </div>

          {result.evidenceHealthDegraded && (
            <p className="batch-panel__blocked-note" role="status">
              <AlertTriangle size={14} aria-hidden="true" />
              {panelCopy.evidenceDegraded}
            </p>
          )}

          <ul className="batch-panel__items" aria-label={panelCopy.itemsAria}>
            {result.items.map((item) => (
              <li key={item.itemId} className="batch-panel__item">
                <span className={`batch-panel__item-status batch-panel__item-status--${item.status}`}>
                  {getBatchItemStatusLabel(item.status, bCopy.itemStatus)}
                </span>
                <span className="batch-panel__item-mod">{item.modId}</span>
                {item.reasonCode !== null && (
                  <span className="batch-panel__item-reason">
                    {getBatchReasonCodeLabel(item.reasonCode, bCopy.reasonCodes)}
                  </span>
                )}
                {item.retryable && (
                  <span className="batch-panel__item-retryable">{panelCopy.retryableBadge}</span>
                )}
              </li>
            ))}
          </ul>
        </div>

        <footer className="batch-panel__footer">
          <button type="button" className="batch-panel__cancel" onClick={onClose}>
            {panelCopy.close}
          </button>
          {canLoadMore && (
            <button type="button" className="batch-panel__more" onClick={onLoadMore}>
              {panelCopy.loadMore}
            </button>
          )}
          {retryAvailableByStatus && (
            <button type="button" className="batch-panel__confirm" onClick={onRetry}>
              <RefreshCw size={16} aria-hidden="true" />
              {panelCopy.retryFailed}
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}

export function BatchModLifecycleRunningPanel({
  workflowState,
  onClose,
}: {
  workflowState: BatchModLifecycleWorkflowState;
  onClose: () => void;
}) {
  const { locale } = useI18n();
  const bCopy = resolveCopy(batchModLifecycleCopy, locale);
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement | null>(null);
  const starting = workflowState.status === "starting";
  useModalFocusTrap({
    active: true,
    containerRef: panelRef,
    // A started batch cannot be cancelled through this panel; keep the modal pinned while
    // the synchronous start is in flight so the attempt identity is not lost.
    closeOnEscape: !starting,
    onRequestClose: onClose,
  });

  const errorCode =
    workflowState.status === "result-error" ? workflowState.errorCode : null;
  const operation =
    workflowState.status === "starting"
      ? (workflowState.request.operation as "install" | "uninstall" | "reinstall")
      : "install";

  return (
    <div className="batch-panel__backdrop" role="presentation">
      <div
        ref={panelRef}
        className="batch-panel batch-panel--preview"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <header className="batch-panel__header">
          <h2 id={titleId}>{getBatchOperationLabel(operation, bCopy.operations)}</h2>
          <button
            type="button"
            className="batch-panel__close"
            aria-label={bCopy.previewPanel.closeAria}
            disabled={starting}
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>
        <div className="batch-panel__body">
          {errorCode !== null ? (
            <div className="batch-panel__error" role="alert">
              <AlertTriangle size={16} aria-hidden="true" />
              {getBatchErrorLabel(errorCode, bCopy)}
            </div>
          ) : (
            <div className="batch-panel__loading" role="status">
              <LoaderCircle size={18} className="batch-panel__spinner" aria-hidden="true" />
              {bCopy.runningPanel.running[operation]}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
