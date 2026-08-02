import { AlertTriangle, CheckCircle2, LoaderCircle, RefreshCw, X } from "lucide-react";
import { useId, useRef } from "react";
import { useModalFocusTrap } from "../../../shared/feedback/useModalFocusTrap";
import {
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
          <h2 id={titleId}>{getBatchOperationLabel(operation)}结果</h2>
          <button
            type="button"
            className="batch-panel__close"
            aria-label="关闭"
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>

        <div className={`batch-panel__attempt batch-panel__attempt--${tone}`} role="status">
          <CheckCircle2 size={16} aria-hidden="true" />
          {getBatchAttemptStatusLabel(result.status)}
          <span className="batch-panel__attempt-id">批次 {batchId}</span>
        </div>

        <div className="batch-panel__summary-counts" aria-label="批量结果汇总">
          <span>成功 {summary.succeededCount}</span>
          <span>失败 {summary.failedCount}</span>
          <span>被阻止 {summary.blockedCount}</span>
          <span>跳过 {summary.skippedCount}</span>
          <span>取消 {summary.cancelledCount}</span>
          {summary.recoveryRequiredCount > 0 && (
            <span>需恢复 {summary.recoveryRequiredCount}</span>
          )}
        </div>

        {result.evidenceHealthDegraded && (
          <p className="batch-panel__blocked-note" role="status">
            <AlertTriangle size={14} aria-hidden="true" />
            部分执行证据健康度下降，请前往恢复中心检查。
          </p>
        )}

        <ul className="batch-panel__items" aria-label="逐项结果">
          {result.items.map((item) => (
            <li key={item.itemId} className="batch-panel__item">
              <span className={`batch-panel__item-status batch-panel__item-status--${item.status}`}>
                {getBatchItemStatusLabel(item.status)}
              </span>
              <span className="batch-panel__item-mod">{item.modId}</span>
              {item.reasonCode !== null && (
                <span className="batch-panel__item-reason">
                  {getBatchReasonCodeLabel(item.reasonCode)}
                </span>
              )}
              {item.retryable && (
                <span className="batch-panel__item-retryable">可重试</span>
              )}
            </li>
          ))}
        </ul>

        <footer className="batch-panel__footer">
          <button type="button" className="batch-panel__cancel" onClick={onClose}>
            关闭
          </button>
          {canLoadMore && (
            <button type="button" className="batch-panel__more" onClick={onLoadMore}>
              加载更多
            </button>
          )}
          {retryAvailableByStatus && (
            <button type="button" className="batch-panel__confirm" onClick={onRetry}>
              <RefreshCw size={16} aria-hidden="true" />
              重试失败项
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
          <h2 id={titleId}>{getBatchOperationLabel(operation)}</h2>
          <button
            type="button"
            className="batch-panel__close"
            aria-label="关闭"
            disabled={starting}
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>
        {errorCode !== null ? (
          <div className="batch-panel__error" role="alert">
            <AlertTriangle size={16} aria-hidden="true" />
            {getBatchErrorLabel(errorCode)}
          </div>
        ) : (
          <div className="batch-panel__loading" role="status">
            <LoaderCircle size={18} className="batch-panel__spinner" aria-hidden="true" />
            正在执行批量{operation === "install" ? "安装" : operation === "uninstall" ? "卸载" : "重装"}…
          </div>
        )}
      </div>
    </div>
  );
}
