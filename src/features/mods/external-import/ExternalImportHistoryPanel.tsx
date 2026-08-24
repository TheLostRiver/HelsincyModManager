import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import {
  ChevronDown,
  ChevronUp,
  CircleAlert,
  FileSearch,
  History,
  LoaderCircle,
  RefreshCcw,
} from "lucide-react";
import { useId } from "react";
import type { ExternalImportHistoryRowViewModel } from "./externalImportHistoryModel";
import type {
  ExternalImportHistoryDetailState,
  ExternalImportHistoryWorkflow,
} from "./useExternalImportHistory";

type ExternalImportHistoryPanelProps = {
  workflow: ExternalImportHistoryWorkflow;
};

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function HistoryRowDetail({
  detailState,
  workflow,
}: {
  detailState: ExternalImportHistoryDetailState;
  workflow: ExternalImportHistoryWorkflow;
}) {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);

  if (detailState.status === "idle") {
    return null;
  }
  if (detailState.status === "loading") {
    return (
      <div className="external-import__state" role="status" aria-live="polite">
        <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
        <span>{extCopy.history.detailLoading}</span>
      </div>
    );
  }
  if (detailState.status === "failed") {
    return (
      <div className="external-import__inline-action is-error" role="alert">
        <span>{detailState.message}</span>
        <button
          type="button"
          className="external-import__button is-secondary"
          onClick={workflow.reloadDetails}
        >
          <RefreshCcw size={15} aria-hidden="true" />
          {extCopy.history.reloadDetail}
        </button>
      </div>
    );
  }

  return (
    <div className="external-import__history-detail">
      <ul className="external-import__result-list">
        {detailState.results.map((result) => (
          <li key={result.candidateId} className="external-import__result-item">
            <div className="external-import__result-main">
              <strong>{result.displayName ?? extCopy.preview.unnamed}</strong>
              <code>{result.candidateId}</code>
              {result.importedModId ? (
                <span>{extCopy.resultPanel.modId(result.importedModId)}</span>
              ) : null}
            </div>
            <div className="external-import__candidate-statuses">
              <span className={`external-import__badge is-${result.statusTone}`}>
                {result.statusLabel}
              </span>
            </div>
            {result.reasonLabel ? (
              <span className="external-import__result-reason">{result.reasonLabel}</span>
            ) : null}
          </li>
        ))}
      </ul>
      {detailState.loadMoreError ? (
        <div className="external-import__load-error" role="alert">
          <CircleAlert size={15} aria-hidden="true" />
          <span>{detailState.loadMoreError}</span>
        </div>
      ) : null}
      {detailState.nextCursor !== null ? (
        <button
          type="button"
          className="external-import__button is-secondary"
          disabled={detailState.loadingMore}
          onClick={workflow.loadMoreDetails}
        >
          {detailState.loadingMore ? (
            <LoaderCircle className="external-import__spinner" size={15} aria-hidden="true" />
          ) : (
            <FileSearch size={15} aria-hidden="true" />
          )}
          {detailState.loadingMore
            ? extCopy.history.loadingMore
            : extCopy.history.loadMoreDetails}
        </button>
      ) : null}
    </div>
  );
}

function HistoryRow({
  row,
  workflow,
}: {
  row: ExternalImportHistoryRowViewModel;
  workflow: ExternalImportHistoryWorkflow;
}) {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const detailState = workflow.detailState;
  const expanded = detailState.status !== "idle" && detailState.batchId === row.batchId;
  const countChips = [
    { value: row.imported, label: extCopy.resultPanel.imported(formatCount(row.imported)) },
    {
      value: row.alreadyImported,
      label: extCopy.resultPanel.alreadyImported(formatCount(row.alreadyImported)),
    },
    { value: row.skipped, label: extCopy.resultPanel.skipped(formatCount(row.skipped)) },
    { value: row.blocked, label: extCopy.resultPanel.blocked(formatCount(row.blocked)) },
    { value: row.failed, label: extCopy.resultPanel.failed(formatCount(row.failed)) },
    { value: row.cancelled, label: extCopy.resultPanel.cancelled(formatCount(row.cancelled)) },
  ].filter((chip) => chip.value > 0);

  return (
    <li className="external-import__history-item" role="listitem">
      <div className="external-import__history-head">
        <div className="external-import__history-meta">
          <strong>{row.createdAtLabel}</strong>
          <span>{row.adapterLabel}</span>
          <span>{extCopy.history.candidateCount(formatCount(row.candidateCount))}</span>
        </div>
        <div className="external-import__candidate-statuses">
          <span className={`external-import__badge is-${row.stateTone}`}>{row.stateLabel}</span>
          {row.hasDetails ? (
            <button
              type="button"
              className="external-import__button is-secondary"
              aria-expanded={expanded}
              onClick={() => workflow.toggleDetails(row.batchId)}
            >
              {expanded ? (
                <ChevronUp size={15} aria-hidden="true" />
              ) : (
                <ChevronDown size={15} aria-hidden="true" />
              )}
              {expanded ? extCopy.history.hideDetails : extCopy.history.viewDetails}
            </button>
          ) : null}
        </div>
      </div>
      {countChips.length > 0 ? (
        <div className="external-import__result-summary">
          {countChips.map((chip) => (
            <span key={chip.label}>{chip.label}</span>
          ))}
        </div>
      ) : (
        <span className="external-import__history-empty-note">{extCopy.history.noResults}</span>
      )}
      {expanded ? <HistoryRowDetail detailState={detailState} workflow={workflow} /> : null}
    </li>
  );
}

export function ExternalImportHistoryPanel({ workflow }: ExternalImportHistoryPanelProps) {
  const { locale } = useI18n();
  const extCopy = resolveCopy(externalImportCopy, locale);
  const headingId = useId();
  const listState = workflow.listState;

  return (
    <section className="external-import__history" aria-labelledby={headingId}>
      <header className="external-import__preview-header">
        <div>
          <span className="external-import__eyebrow">
            <History size={13} aria-hidden="true" />
            {extCopy.history.title}
          </span>
          <h3 id={headingId}>
            {listState.status === "ready"
              ? extCopy.history.loadedCount(
                  formatCount(listState.rows.length),
                  formatCount(listState.totalCount),
                )
              : extCopy.history.title}
          </h3>
        </div>
        <button
          type="button"
          className="external-import__button is-secondary"
          disabled={listState.status === "loading"}
          onClick={workflow.refresh}
        >
          <RefreshCcw size={15} aria-hidden="true" />
          {extCopy.history.reload}
        </button>
      </header>

      {listState.status === "idle" || listState.status === "loading" ? (
        <div className="external-import__state" role="status" aria-live="polite">
          <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
          <span>{extCopy.history.loading}</span>
        </div>
      ) : null}

      {listState.status === "failed" ? (
        <div className="external-import__inline-action is-error" role="alert">
          <span>{listState.message}</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.refresh}
          >
            <RefreshCcw size={15} aria-hidden="true" />
            {extCopy.history.reload}
          </button>
        </div>
      ) : null}

      {listState.status === "empty" ? (
        <div className="external-import__empty" role="status" aria-live="polite">
          <FileSearch size={24} aria-hidden="true" />
          <strong>{extCopy.history.emptyTitle}</strong>
          <span>{extCopy.history.emptyHint}</span>
        </div>
      ) : null}

      {listState.status === "ready" ? (
        <>
          <ul className="external-import__history-list" role="list">
            {listState.rows.map((row) => (
              <HistoryRow key={row.batchId} row={row} workflow={workflow} />
            ))}
          </ul>
          {listState.loadMoreError ? (
            <div className="external-import__load-error" role="alert">
              <CircleAlert size={15} aria-hidden="true" />
              <span>{listState.loadMoreError}</span>
            </div>
          ) : null}
          {listState.nextCursor !== null ? (
            <div className="external-import__result-actions">
              <button
                type="button"
                className="external-import__button is-secondary"
                disabled={listState.loadingMore}
                onClick={workflow.loadMore}
              >
                {listState.loadingMore ? (
                  <LoaderCircle className="external-import__spinner" size={15} aria-hidden="true" />
                ) : (
                  <FileSearch size={15} aria-hidden="true" />
                )}
                {listState.loadingMore ? extCopy.history.loadingMore : extCopy.history.loadMore}
              </button>
            </div>
          ) : null}
        </>
      ) : null}

      <p className="external-import__history-hints">
        <span>{extCopy.history.retentionHint}</span>
        <span>{extCopy.history.retryHint}</span>
      </p>
    </section>
  );
}
