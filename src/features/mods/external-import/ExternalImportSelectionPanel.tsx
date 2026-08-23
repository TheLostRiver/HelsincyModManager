import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import {
  CheckCircle2,
  CircleAlert,
  FileSearch,
  LoaderCircle,
  RefreshCcw,
  XCircle,
} from "lucide-react";
import { useId } from "react";
import { ExternalImportCandidateSelectionItem } from "./ExternalImportCandidateSelectionItem";
import { ExternalImportResultPanel } from "./ExternalImportResultPanel";
import { getExternalImportPhaseLabel } from "./externalImportProgressState";
import type { ExternalImportSelectionWorkflow } from "./useExternalImportSelectionWorkflow";

type ExternalImportSelectionPanelProps = {
  workflow: ExternalImportSelectionWorkflow;
};

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function ImportProgress({
  workflow,
}: {
  workflow: ExternalImportSelectionWorkflow;
}) {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const state = workflow.importState;
  if (state.status === "idle") {
    return null;
  }
  if (state.status === "starting") {
    return (
      <div className="external-import__state" role="status" aria-live="polite">
        <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
        <span>{extCopy.selectionPanel.sealing}</span>
      </div>
    );
  }
  if (state.status === "running") {
    const progress =
      state.current !== null && state.total !== null
        ? extCopy.selectionPanel.progressCount(formatCount(state.current), formatCount(state.total))
        : "";
    return (
      <div className="external-import__import-progress" role="status" aria-live="polite">
        <div>
          <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
          <span>{getExternalImportPhaseLabel(state.phase, extCopy.progress)}{progress}</span>
        </div>
        <button
          type="button"
          className="external-import__button is-danger"
          disabled={workflow.cancelPending}
          onClick={workflow.cancelImport}
        >
          {workflow.cancelPending ? (
            <LoaderCircle className="external-import__spinner" size={15} />
          ) : (
            <XCircle size={15} />
          )}
          {workflow.cancelPending ? extCopy.selectionPanel.cancelPending : extCopy.selectionPanel.cancelImport}
        </button>
      </div>
    );
  }
  if (state.status === "cancelling") {
    return (
      <div className="external-import__state is-muted" role="status" aria-live="polite">
        <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
        <span>{extCopy.selectionPanel.cancellingSafely}</span>
      </div>
    );
  }
  if (state.status === "completed") {
    return (
      <div className="external-import__state is-success" role="status" aria-live="polite">
        <CheckCircle2 size={18} aria-hidden="true" />
        <span>{extCopy.selectionPanel.completedReadingResults}</span>
      </div>
    );
  }
  if (state.status === "cancelled") {
    return (
      <div className="external-import__state is-muted" role="status" aria-live="polite">
        <XCircle size={18} aria-hidden="true" />
        <span>{extCopy.selectionPanel.cancelledNoInference}</span>
      </div>
    );
  }
  return (
    <div className="external-import__state is-error" role="alert">
      <CircleAlert size={18} aria-hidden="true" />
      <span>{extCopy.selectionPanel.incompleteReadingResults}</span>
    </div>
  );
}

export function ExternalImportSelectionPanel({
  workflow,
}: ExternalImportSelectionPanelProps) {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const headingId = useId();
  const categories =
    workflow.categoryState.status === "ready" ? workflow.categoryState.options : [];
  const selectionBusy = workflow.pendingAction !== null;

  return (
    <section className="external-import__selection" aria-labelledby={headingId}>
      <header className="external-import__preview-header">
        <div>
          <span className="external-import__eyebrow">{extCopy.selectionPanel.candidateEyebrow}</span>
          <h3 id={headingId}>
            {workflow.selection
              ? extCopy.selectionPanel.selectedCount(formatCount(workflow.selection.selectedCount))
              : extCopy.selectionPanel.creatingSnapshot}
          </h3>
        </div>
        {workflow.selection?.status === "editing" && workflow.selectionEditable ? (
          <span className="external-import__badge is-neutral">{extCopy.selectionPanel.editable}</span>
        ) : workflow.selection ? (
          <span className="external-import__badge is-warning">
            {workflow.selection.status === "sealed" ? extCopy.selectionPanel.sealed : extCopy.selectionPanel.expired}
          </span>
        ) : null}
      </header>

      {workflow.listenerStatus === "failed" ? (
        <div className="external-import__inline-action is-error" role="alert">
          <span>{extCopy.selectionPanel.progressListenerUnavailable}</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.retryListener}
          >
            <RefreshCcw size={15} />
            {extCopy.selectionPanel.retryProgressListener}
          </button>
        </div>
      ) : null}

      {workflow.categoryState.status === "failed" ? (
        <div className="external-import__inline-action is-error" role="alert">
          <span>{workflow.categoryState.message}</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.retryCategories}
          >
            <RefreshCcw size={15} />
            {extCopy.selectionPanel.reloadCategories}
          </button>
        </div>
      ) : null}

      {workflow.selectionError ? (
        <div className="external-import__load-error" role="alert">
          <CircleAlert size={15} aria-hidden="true" />
          <span>{workflow.selectionError}</span>
        </div>
      ) : null}

      {workflow.previewState.status === "loading" ? (
        <div className="external-import__state" role="status" aria-live="polite">
          <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
          <span>{extCopy.selectionPanel.loadingSnapshot}</span>
        </div>
      ) : null}

      {workflow.previewState.status === "failed" ? (
        <div className="external-import__inline-action is-error" role="alert">
          <span>{workflow.previewState.message}</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.retryPreview}
          >
            <RefreshCcw size={15} />
            {extCopy.selectionPanel.reloadCandidates}
          </button>
        </div>
      ) : null}

      {workflow.previewState.status === "empty" ? (
        <div className="external-import__empty" role="status" aria-live="polite">
          <FileSearch size={24} aria-hidden="true" />
          <strong>{extCopy.selectionPanel.noCandidates}</strong>
          <span>{extCopy.selectionPanel.scanReturned(formatCount(workflow.previewState.totalCount))}</span>
        </div>
      ) : null}

      {workflow.previewState.status === "ready" ? (
        <>
          <div className="external-import__selection-toolbar">
            <span>
              {extCopy.selectionPanel.loadedCount(
                formatCount(workflow.previewState.candidates.length),
                formatCount(workflow.previewState.totalCount),
              )}
            </span>
            <button
              type="button"
              className="external-import__button is-secondary"
              disabled={!workflow.selectionEditable || selectionBusy}
              onClick={workflow.selectAll}
            >
              {workflow.pendingAction === "select-all" ? (
                <LoaderCircle className="external-import__spinner" size={15} />
              ) : (
                <CheckCircle2 size={15} />
              )}
              {extCopy.selectionPanel.selectAllImportable}
            </button>
          </div>

          <ul className="external-import__candidate-list">
            {workflow.previewState.candidates.map((candidate) => (
              <ExternalImportCandidateSelectionItem
                key={candidate.candidateId}
                candidate={candidate}
                categories={categories}
                decision={
                  workflow.decisionDrafts[candidate.candidateId] ?? {
                    conflictResolution: null,
                    categoryId: null,
                  }
                }
                disabled={!workflow.selectionEditable || selectionBusy}
                pending={
                  typeof workflow.pendingAction === "object" &&
                  workflow.pendingAction?.candidateId === candidate.candidateId
                }
                onDecisionChange={(decision) =>
                  workflow.setCandidateDecision(candidate.candidateId, decision)
                }
                onSelectedChange={(selected) =>
                  workflow.setCandidateSelected(candidate.candidateId, selected)
                }
              />
            ))}
          </ul>

          {workflow.previewState.loadMoreError ? (
            <div className="external-import__load-error" role="alert">
              <CircleAlert size={15} aria-hidden="true" />
              <span>{workflow.previewState.loadMoreError}</span>
            </div>
          ) : null}

          {workflow.previewState.nextCursor !== null ? (
            <button
              type="button"
              className="external-import__button is-secondary external-import__load-more"
              disabled={
                workflow.previewState.loadingMore ||
                selectionBusy ||
                !workflow.selectionEditable
              }
              onClick={workflow.loadMore}
            >
              {workflow.previewState.loadingMore ? (
                <LoaderCircle className="external-import__spinner" size={15} />
              ) : (
                <FileSearch size={15} />
              )}
              {workflow.previewState.loadingMore ? extCopy.selectionPanel.loadingMore : extCopy.selectionPanel.loadMoreCandidates}
            </button>
          ) : null}
        </>
      ) : null}

      {workflow.selectionEditable && workflow.selection ? (
        <div className="external-import__start-row">
          <div>
            <strong>{extCopy.selectionPanel.importOnlyTitle}</strong>
            <span>{extCopy.selectionPanel.importOnlyDescription}</span>
          </div>
          <button
            type="button"
            className="external-import__button is-primary"
            disabled={
              selectionBusy ||
              workflow.listenerStatus !== "ready" ||
              workflow.selection.selectedCount === 0
            }
            onClick={workflow.startImport}
          >
            {workflow.pendingAction === "start" ? (
              <LoaderCircle className="external-import__spinner" size={15} />
            ) : (
              <CheckCircle2 size={15} />
            )}
            {extCopy.selectionPanel.startBatchImport}
          </button>
        </div>
      ) : null}

      <ImportProgress workflow={workflow} />
      <ExternalImportResultPanel workflow={workflow.result} />
    </section>
  );
}
