import { AlertTriangle, CheckCircle2, LoaderCircle, X } from "lucide-react";
import { useId, useRef } from "react";
import { useModalFocusTrap } from "../../../shared/feedback/useModalFocusTrap";
import { resolveCopy, useI18n } from "../../../shared/i18n";
import type {
  BatchModLifecycleExecutionPolicy,
  BatchModLifecyclePreviewDto,
  BatchModLifecycleRequestDto,
} from "./batchModLifecycleTypes";
import {
  batchModLifecycleCopy,
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
  onReplacementTargetChange: (modId: string, targetId: string) => void;
  onPreviewWithReplacementTargets: () => void;
  onConfirm: () => void;
  onClose: () => void;
};

function operationOf(state: BatchModLifecycleWorkflowState): "install" | "uninstall" | "reinstall" {
  if (state.status === "target-selection") {
    return state.operation;
  }
  if (state.status === "preview-ready" || state.status === "confirming") {
    return state.request.operation as "install" | "uninstall" | "reinstall";
  }
  if (state.status === "preview-error") {
    return state.operation;
  }
  return "install";
}

function PreviewSummary({ preview }: { preview: BatchModLifecyclePreviewDto }) {
  const { locale } = useI18n();
  const panelCopy = resolveCopy(batchModLifecycleCopy, locale).previewPanel;
  const summary = preview.actionSummary;
  return (
    <section className="batch-panel__summary" aria-label={panelCopy.summaryAria}>
      <div className="batch-panel__summary-counts">
        <span>{panelCopy.totalCount(preview.readyItemCount + preview.blockedItemCount)}</span>
        <span>{panelCopy.readyCount(preview.readyItemCount)}</span>
        {preview.blockedItemCount > 0 && <span>{panelCopy.blockedCount(preview.blockedItemCount)}</span>}
      </div>
      <div className="batch-panel__summary-actions">
        <span>{panelCopy.addedCount(summary.added)}</span>
        <span>{panelCopy.retainedCount(summary.retained)}</span>
        <span>{panelCopy.replacedCount(summary.replaced)}</span>
        <span>{panelCopy.staleCount(summary.stale)}</span>
        <span>{panelCopy.actionCount(summary.actions)}</span>
      </div>
      {preview.blockedItemCount > 0 && (
        <p className="batch-panel__blocked-note" role="status">
          <AlertTriangle size={14} aria-hidden="true" />
          {panelCopy.blockedNote(preview.blockedItemCount)}
        </p>
      )}
    </section>
  );
}

function PreviewItems({ request }: { request: BatchModLifecycleRequestDto }) {
  const { locale } = useI18n();
  const panelCopy = resolveCopy(batchModLifecycleCopy, locale).previewPanel;
  const replacementTargetByModId = new Map(
    (request.replacementTargets ?? []).map((target) => [target.modId, target.targetId]),
  );

  return (
    <section className="batch-panel__preview-items" aria-label={panelCopy.itemsAria}>
      <h3>{panelCopy.itemsTitle}</h3>
      <ul>
        {request.items.map((item) => (
          <li className="batch-panel__preview-item" key={item.modId}>
            <strong>{item.modId}</strong>
            <dl>
              {item.operation === "install" && (
                <>
                  <div>
                    <dt>{panelCopy.displayRevision}</dt>
                    <dd>{item.revisionId}</dd>
                  </div>
                  <div>
                    <dt>{panelCopy.layerLabel}</dt>
                    <dd>{item.layer.name}/{item.layer.priority}</dd>
                  </div>
                </>
              )}
              {item.operation === "uninstall" && (
                <div>
                  <dt>{panelCopy.installedRevision}</dt>
                  <dd>{item.expectedInstalledRevisionId}</dd>
                </div>
              )}
              {item.operation === "reinstall" && (
                <>
                  <div>
                    <dt>{panelCopy.installedRevision}</dt>
                    <dd>{item.installedRevisionId}</dd>
                  </div>
                  <div>
                    <dt>{panelCopy.candidateDisplayRevision}</dt>
                    <dd>{item.candidateRevisionId}</dd>
                  </div>
                  <div>
                    <dt>{panelCopy.layerLabel}</dt>
                    <dd>{item.layer.name}/{item.layer.priority}</dd>
                  </div>
                  <div>
                    <dt>{panelCopy.targetLabel}</dt>
                    <dd>
                      {replacementTargetByModId.has(item.modId)
                        ? panelCopy.switchTo(replacementTargetByModId.get(item.modId) ?? "")
                        : panelCopy.keepCurrent}
                    </dd>
                  </div>
                </>
              )}
            </dl>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function BatchModLifecyclePreviewPanel({
  workflowState,
  resolution,
  policy,
  onPolicyChange,
  onReplacementTargetChange,
  onPreviewWithReplacementTargets,
  onConfirm,
  onClose,
}: BatchModLifecyclePreviewPanelProps) {
  const { locale } = useI18n();
  const bCopy = resolveCopy(batchModLifecycleCopy, locale);
  const panelCopy = bCopy.previewPanel;
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
  const targetSelection = workflowState.status === "target-selection" ? workflowState : null;
  const errorCode =
    workflowState.status === "preview-error" ? workflowState.errorCode : null;
  const targetSelectionReady =
    targetSelection !== null
    && targetSelection.targetFacts.every((facts) => {
      const targetId = targetSelection.selectedTargets[facts.modId];
      return facts.retargetable
        && targetId !== null
        && targetId !== facts.installedTargetId
        && facts.targets.some((target) => target.id === targetId);
    });
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
          <h2 id={titleId}>{getBatchOperationLabel(operation, bCopy.operations)}</h2>
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
          {loading && (
            <div className="batch-panel__loading" role="status">
              <LoaderCircle size={18} className="batch-panel__spinner" aria-hidden="true" />
              {panelCopy.generating}
            </div>
          )}

          {errorCode !== null && (
            <div className="batch-panel__error" role="alert">
              <AlertTriangle size={16} aria-hidden="true" />
              {getBatchErrorLabel(errorCode, bCopy)}
            </div>
          )}

          {targetSelection !== null && (
            <section className="batch-panel__target-selection" aria-label={panelCopy.targetSelectionAria}>
              <div>
                <h3>{panelCopy.targetSelectionTitle}</h3>
                <p>{panelCopy.targetSelectionHint}</p>
              </div>
              {targetSelection.targetFacts.map((facts) => {
                const availableTargets = facts.targets.filter(
                  (target) => target.id !== facts.installedTargetId,
                );
                return (
                  <fieldset className="batch-panel__target-group" key={facts.modId}>
                    <legend>{facts.modId}</legend>
                    {!facts.retargetable || availableTargets.length === 0 ? (
                      <p className="batch-panel__target-unavailable" role="alert">
                        {panelCopy.targetUnavailable}
                      </p>
                    ) : (
                      <div className="batch-panel__target-options" role="radiogroup" aria-label={panelCopy.targetGroupAria(facts.modId)}>
                        {availableTargets.map((target) => (
                          <label className="batch-panel__target-option" key={target.id}>
                            <input
                              type="radio"
                              name={`batch-replacement-target-${facts.modId}`}
                              value={target.id}
                              checked={targetSelection.selectedTargets[facts.modId] === target.id}
                              onChange={() => onReplacementTargetChange(facts.modId, target.id)}
                            />
                            <span>
                              <strong>{target.displayName}</strong>
                              {target.secondaryName ? <small>{target.secondaryName}</small> : null}
                            </span>
                          </label>
                        ))}
                      </div>
                    )}
                  </fieldset>
                );
              })}
            </section>
          )}

          {preview !== null && workflowState.status === "preview-ready" && (
            <>
              <PreviewSummary preview={preview} />
              <PreviewItems request={workflowState.request} />
              {resolution.excluded.length > 0 && (
                <section className="batch-panel__excluded" aria-label={panelCopy.excludedTitle}>
                  <h3>{panelCopy.excludedTitle}</h3>
                  <ul>
                    {resolution.excluded.map(({ modId, reason }) => (
                      <li key={modId}>
                        {panelCopy.excludedItem(modId, getBatchExcludedReasonLabel(reason, bCopy))}
                      </li>
                    ))}
                  </ul>
                </section>
              )}
              {resolution.unresolvable.length > 0 && (
                <section className="batch-panel__excluded" aria-label={panelCopy.unresolvableAria}>
                  <h3>{panelCopy.unresolvableTitle}</h3>
                  <ul>
                    {resolution.unresolvable.map((modId) => (
                      <li key={modId}>{panelCopy.unresolvableItem(modId)}</li>
                    ))}
                  </ul>
                </section>
              )}
            </>
          )}

          {(preview !== null || targetSelection !== null) && (
            <section className="batch-panel__policy" aria-label={panelCopy.policyTitle}>
              <h3>{panelCopy.policyTitle}</h3>
              <label className="batch-panel__policy-option">
                <input
                  type="radio"
                  name="execution-policy"
                  checked={policy === "stop_on_failure"}
                  onChange={() => onPolicyChange("stop_on_failure")}
                />
                <span>{panelCopy.stopOnFailure}</span>
              </label>
              <label className="batch-panel__policy-option">
                <input
                  type="radio"
                  name="execution-policy"
                  checked={policy === "continue_on_item_failure"}
                  onChange={() => onPolicyChange("continue_on_item_failure")}
                />
                <span>{panelCopy.continueOnFailure}</span>
              </label>
            </section>
          )}
        </div>

        <footer className="batch-panel__footer">
          <button type="button" className="batch-panel__cancel" onClick={onClose}>
            {panelCopy.cancel}
          </button>
          {targetSelection !== null ? (
            <button
              type="button"
              className="batch-panel__confirm"
              disabled={!targetSelectionReady}
              onClick={onPreviewWithReplacementTargets}
            >
              <CheckCircle2 size={16} aria-hidden="true" />
              {panelCopy.generatePreview}
            </button>
          ) : (
            <button
              type="button"
              className="batch-panel__confirm"
              disabled={confirmDisabled}
              onClick={onConfirm}
            >
              <CheckCircle2 size={16} aria-hidden="true" />
              {panelCopy.confirmStart}
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}
