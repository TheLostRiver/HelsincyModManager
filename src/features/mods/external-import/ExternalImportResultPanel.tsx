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
  const headingId = useId();
  const state = workflow.state;
  if (state.status === "idle") {
    return null;
  }

  if (state.status === "loading") {
    return (
      <section className="external-import__results" aria-labelledby={headingId}>
        <h3 id={headingId} className="external-import__visually-hidden">
          批量导入结果
        </h3>
        <div className="external-import__state" role="status" aria-live="polite">
          <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
          <span>正在读取服务端确认的结果明细</span>
        </div>
      </section>
    );
  }

  if (state.status === "failed") {
    return (
      <section className="external-import__results" aria-labelledby={headingId}>
        <h3 id={headingId} className="external-import__visually-hidden">
          批量导入结果
        </h3>
        <div className="external-import__inline-action is-error" role="alert">
          <span>{state.message}</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.retryResultQuery}
          >
            <RefreshCcw size={15} aria-hidden="true" />
            重新读取结果
          </button>
        </div>
      </section>
    );
  }

  const batchStatus = getExternalImportBatchStatusLabel(state.batchStatus);
  return (
    <section className="external-import__results" aria-labelledby={headingId}>
      <header className="external-import__preview-header">
        <div>
          <span className="external-import__eyebrow">导入结果</span>
          <h3 id={headingId}>
            {state.status === "ready"
              ? `已载入 ${formatCount(state.results.length)} / ${formatCount(state.totalCount)} 项`
              : "当前批次没有结果项"}
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
          <span>正在重试可恢复项；下方是上一次权威结果，任务结束后会自动刷新。</span>
        </div>
      ) : null}

      {state.status === "empty" ? (
        <div className="external-import__empty" role="status" aria-live="polite">
          <FileSearch size={24} aria-hidden="true" />
          <strong>没有结果明细</strong>
          <span>批次状态已由后端确认，没有可分页的候选结果。</span>
        </div>
      ) : (
        <>
          <div className="external-import__result-summary" aria-label="当前已载入结果汇总">
            <span>已导入 {formatCount(workflow.summary.imported)}</span>
            <span>已存在 {formatCount(workflow.summary.alreadyImported)}</span>
            <span>已跳过 {formatCount(workflow.summary.skipped)}</span>
            <span>已阻断 {formatCount(workflow.summary.blocked)}</span>
            <span>失败 {formatCount(workflow.summary.failed)}</span>
            <span>取消 {formatCount(workflow.summary.cancelled)}</span>
          </div>

          <ul className="external-import__result-list">
            {state.results.map((result) => (
              <li key={result.candidateId} className="external-import__result-item">
                <div className="external-import__result-main">
                  <strong>候选结果</strong>
                  <code>{result.candidateId}</code>
                  {result.importedModId ? <span>Mod ID：{result.importedModId}</span> : null}
                </div>
                <div className="external-import__candidate-statuses">
                  <span
                    className={`external-import__badge is-${result.statusTone}`}
                  >
                    {result.statusLabel}
                  </span>
                  {result.retryable ? (
                    <span className="external-import__badge is-warning">可重试</span>
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
            {state.loadingMore ? "正在载入" : "载入更多结果"}
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
            {workflow.retryPending ? "正在创建重试任务" : "重试可恢复项"}
          </button>
        ) : null}
      </div>
    </section>
  );
}
