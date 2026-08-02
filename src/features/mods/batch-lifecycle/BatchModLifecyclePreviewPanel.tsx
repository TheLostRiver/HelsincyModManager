import { AlertTriangle, CheckCircle2, LoaderCircle, X } from "lucide-react";
import { useId, useRef } from "react";
import { useModalFocusTrap } from "../../../shared/feedback/useModalFocusTrap";
import type {
  BatchModLifecycleExecutionPolicy,
  BatchModLifecyclePreviewDto,
} from "./batchModLifecycleTypes";
import {
  getBatchErrorLabel,
  getBatchExcludedReasonLabel,
  getBatchOperationLabel,
} from "./batchModLifecycleCopy";
import type { BatchModLifecycleItemResolution } from "./batchModLifecycleWorkflow";
import type { BatchModLifecycleWorkflowState } from "./batchModLifecycleWorkflow";
import "./BatchModLifecyclePanel.css";

export type BatchModLifecyclePreviewPanelProps = {
  workflowState: BatchModLifecycleWorkflowState;
  resolution: BatchModLifecycleItemResolution;
  policy: BatchModLifecycleExecutionPolicy;
  onPolicyChange: (policy: BatchModLifecycleExecutionPolicy) => void;
  onConfirm: () => void;
  onClose: () => void;
};

function operationOf(state: BatchModLifecycleWorkflowState): "install" | "uninstall" | "reinstall" {
  if (state.status === "preview-ready" || state.status === "confirming") {
    return state.request.operation as "install" | "uninstall" | "reinstall";
  }
  if (state.status === "preview-error") {
    return state.operation;
  }
  return "install";
}

function PreviewSummary({ preview }: { preview: BatchModLifecyclePreviewDto }) {
  const summary = preview.actionSummary;
  return (
    <section className="batch-panel__summary" aria-label="批量计划摘要">
      <div className="batch-panel__summary-counts">
        <span>共 {preview.readyItemCount + preview.blockedItemCount} 项</span>
        <span>可执行 {preview.readyItemCount} 项</span>
        {preview.blockedItemCount > 0 && <span>被阻止 {preview.blockedItemCount} 项</span>}
      </div>
      <div className="batch-panel__summary-actions">
        <span>新增 {summary.added}</span>
        <span>保留 {summary.retained}</span>
        <span>替换 {summary.replaced}</span>
        <span>过期 {summary.stale}</span>
        <span>动作 {summary.actions}</span>
      </div>
      {preview.blockedItemCount > 0 && (
        <p className="batch-panel__blocked-note" role="status">
          <AlertTriangle size={14} aria-hidden="true" />
          {preview.blockedItemCount} 项因版本或目标冲突被阻止；继续执行时将跳过这些项。
        </p>
      )}
    </section>
  );
}

export function BatchModLifecyclePreviewPanel({
  workflowState,
  resolution,
  policy,
  onPolicyChange,
  onConfirm,
  onClose,
}: BatchModLifecyclePreviewPanelProps) {
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement | null>(null);
  useModalFocusTrap({
    active: true,
    containerRef: panelRef,
    closeOnEscape: true,
    onRequestClose: onClose,
  });
  const operation = operationOf(workflowState);
  const loading =
    workflowState.status === "resolving"
    || workflowState.status === "preview-loading"
    || workflowState.status === "confirming";
  const preview = workflowState.status === "preview-ready" ? workflowState.preview : null;
  const errorCode =
    workflowState.status === "preview-error" ? workflowState.errorCode : null;
  const confirmDisabled =
    workflowState.status !== "preview-ready"
    || preview === null
    || preview.previewToken === null;

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
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>

        {loading && (
          <div className="batch-panel__loading" role="status">
            <LoaderCircle size={18} className="batch-panel__spinner" aria-hidden="true" />
            正在生成批量计划…
          </div>
        )}

        {errorCode !== null && (
          <div className="batch-panel__error" role="alert">
            <AlertTriangle size={16} aria-hidden="true" />
            {getBatchErrorLabel(errorCode)}
          </div>
        )}

        {preview !== null && (
          <>
            <PreviewSummary preview={preview} />
            {resolution.excluded.length > 0 && (
              <section className="batch-panel__excluded" aria-label="不参与本次操作的项">
                <h3>不参与本次操作的项</h3>
                <ul>
                  {resolution.excluded.map(({ modId, reason }) => (
                    <li key={modId}>
                      {modId}：{getBatchExcludedReasonLabel(reason)}
                    </li>
                  ))}
                </ul>
              </section>
            )}
            {resolution.unresolvable.length > 0 && (
              <section className="batch-panel__excluded" aria-label="无法解析的项">
                <h3>无法解析版本的项</h3>
                <ul>
                  {resolution.unresolvable.map((modId) => (
                    <li key={modId}>{modId}：无法读取版本信息</li>
                  ))}
                </ul>
              </section>
            )}
            <section className="batch-panel__policy" aria-label="执行策略">
              <h3>执行策略</h3>
              <label className="batch-panel__policy-option">
                <input
                  type="radio"
                  name="execution-policy"
                  checked={policy === "stop_on_failure"}
                  onChange={() => onPolicyChange("stop_on_failure")}
                />
                <span>遇到失败即停止（推荐）</span>
              </label>
              <label className="batch-panel__policy-option">
                <input
                  type="radio"
                  name="execution-policy"
                  checked={policy === "continue_on_item_failure"}
                  onChange={() => onPolicyChange("continue_on_item_failure")}
                />
                <span>跳过失败项继续</span>
              </label>
            </section>
          </>
        )}

        <footer className="batch-panel__footer">
          <button type="button" className="batch-panel__cancel" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="batch-panel__confirm"
            disabled={confirmDisabled}
            onClick={onConfirm}
          >
            <CheckCircle2 size={16} aria-hidden="true" />
            确认并开始
          </button>
        </footer>
      </div>
    </div>
  );
}
