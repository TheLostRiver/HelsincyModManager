import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import {
  CheckCircle2,
  CircleAlert,
  FileSearch,
  LoaderCircle,
  RefreshCcw,
  RotateCw,
} from "lucide-react";
import { useId } from "react";
import {
  getExternalImportBatchStatusLabel,
} from "./externalImportResultModel";
import type { ExternalImportResultWorkflow } from "./useExternalImportResultWorkflow";

type ExternalImportResultPanelProps = {
  workflow: ExternalImportResultWorkflow;
};

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

export function ExternalImportResultPanel({
  workflow,
}: ExternalImportResultPanelProps) {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const headingId = useId();
  const state = workflow.state;
  if (state.status === "idle") {
    return null;
  }

  if (state.status === "loading") {
    return (
      <section className="external-import__results" aria-labelledby={headingId}>
        <h3 id={headingId} className="external-import__visually-hidden">
          {extCopy.resultPanel.title}
        </h3>
        <div className="external-import__state" role="status" aria-live="polite">
          <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
          <span>{extCopy.resultPanel.readingDetails}</span>
        </div>
      </section>
    );
  }

  if (state.status === "failed") {
    return (
      <section className="external-import__results" aria-labelledby={headingId}>
        <h3 id={headingId} className="external-import__visually-hidden">
          {extCopy.resultPanel.title}
        </h3>
        <div className="external-import__inline-action is-error" role="alert">
          <span>{state.message}</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.retryResultQuery}
          >
            <RefreshCcw size={15} aria-hidden="true" />
            {extCopy.resultPanel.reloadResults}
          </button>
        </div>
      </section>
    );
  }

  const batchStatus = getExternalImportBatchStatusLabel(state.batchStatus, extCopy.result);
  return (
    <section className="external-import__results" aria-labelledby={headingId}>
      <header className="external-import__preview-header">
        <div>
          <span className="external-import__eyebrow">{extCopy.resultPanel.resultEyebrow}</span>
          <h3 id={headingId}>
            {state.status === "ready"
              ? extCopy.resultPanel.loadedCount(formatCount(state.results.length), formatCount(state.totalCount))
              : extCopy.resultPanel.emptyBatch}
          </h3>
        </div>
        <span
          className={`external-import__badge ${
            state.batchStatus === "completed"
              ? "is-ready"
              : state.batchStatus === "completed_with_errors"
                ? "is-warning"
                : "is-danger"
          }`}
        >
          {batchStatus}
        </span>
      </header>

      {workflow.resultStale ? (
        <div className="external-import__state is-muted" role="status" aria-live="polite">
          <RotateCw className="external-import__spinner" size={18} aria-hidden="true" />
          <span>{extCopy.resultPanel.retryingHint}</span>
        </div>
      ) : null}

      {state.status === "empty" ? (
        <div className="external-import__empty" role="status" aria-live="polite">
          <FileSearch size={24} aria-hidden="true" />
          <strong>{extCopy.resultPanel.noDetailsTitle}</strong>
          <span>{extCopy.resultPanel.noDetailsBody}</span>
        </div>
      ) : (
        <>
          <div className="external-import__result-summary" aria-label={extCopy.resultPanel.summaryAria}>
            <span>{extCopy.resultPanel.imported(formatCount(workflow.summary.imported))}</span>
            <span>{extCopy.resultPanel.alreadyImported(formatCount(workflow.summary.alreadyImported))}</span>
            <span>{extCopy.resultPanel.skipped(formatCount(workflow.summary.skipped))}</span>
            <span>{extCopy.resultPanel.blocked(formatCount(workflow.summary.blocked))}</span>
            <span>{extCopy.resultPanel.failed(formatCount(workflow.summary.failed))}</span>
            <span>{extCopy.resultPanel.cancelled(formatCount(workflow.summary.cancelled))}</span>
          </div>

          <ul className="external-import__result-list">
            {state.results.map((result) => (
              <li key={result.candidateId} className="external-import__result-item">
                <div className="external-import__result-main">
                  <strong>{extCopy.resultPanel.candidateResult}</strong>
                  <code>{result.candidateId}</code>
                  {result.importedModId ? <span>{extCopy.resultPanel.modId(result.importedModId)}</span> : null}
                </div>
                <div className="external-import__candidate-statuses">
                  <span
                    className={`external-import__badge is-${result.statusTone}`}
                  >
                    {result.statusLabel}
                  </span>
                  {result.retryable ? (
                    <span className="external-import__badge is-warning">{extCopy.resultPanel.retryableBadge}</span>
                  ) : null}
                </div>
                {result.reasonLabel ? (
                  <span className="external-import__result-reason">
                    {result.reasonLabel}
                  </span>
                ) : null}
              </li>
            ))}
          </ul>

          {state.loadMoreError ? (
            <div className="external-import__load-error" role="alert">
              <CircleAlert size={15} aria-hidden="true" />
              <span>{state.loadMoreError}</span>
            </div>
          ) : null}
        </>
      )}

      {workflow.actionError ? (
        <div className="external-import__load-error" role="alert">
          <CircleAlert size={15} aria-hidden="true" />
          <span>{workflow.actionError}</span>
        </div>
      ) : null}

      <div className="external-import__result-actions">
        {state.status === "ready" && state.nextCursor !== null ? (
          <button
            type="button"
            className="external-import__button is-secondary"
            disabled={state.loadingMore || workflow.retryPending || workflow.resultStale}
            onClick={workflow.loadMore}
          >
            {state.loadingMore ? (
              <LoaderCircle className="external-import__spinner" size={15} aria-hidden="true" />
            ) : (
              <FileSearch size={15} aria-hidden="true" />
            )}
            {state.loadingMore ? extCopy.resultPanel.loadingMore : extCopy.resultPanel.loadMoreResults}
          </button>
        ) : null}
        {workflow.retryAvailable ? (
          <button
            type="button"
            className="external-import__button is-primary"
            disabled={
              (state.status === "ready" && state.loadingMore) ||
              workflow.retryPending ||
              workflow.resultStale
            }
            onClick={workflow.retryResults}
          >
            {workflow.retryPending ? (
              <LoaderCircle className="external-import__spinner" size={15} aria-hidden="true" />
            ) : (
              <CheckCircle2 size={15} aria-hidden="true" />
            )}
            {workflow.retryPending ? extCopy.resultPanel.creatingRetryTask : extCopy.resultPanel.retryRecoverable}
          </button>
        ) : null}
      </div>
    </section>
  );
}
