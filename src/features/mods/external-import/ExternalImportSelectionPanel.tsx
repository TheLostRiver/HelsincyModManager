import { resolveCopy, useI18n } from "../../../shared/i18n";
import { externalImportCopy } from "./externalImportCopy";
import {
  CheckCircle2,
  CircleAlert,
  CircleSlash,
  FileSearch,
  LoaderCircle,
  RefreshCcw,
  XCircle,
} from "lucide-react";
import { useId } from "react";
import { History, X } from "lucide-react";
import { ExternalImportCandidateSelectionItem } from "./ExternalImportCandidateSelectionItem";
import { ExternalImportResultPanel } from "./ExternalImportResultPanel";
import { getExternalImportPhaseLabel } from "./externalImportProgressState";
import type { ExternalImportSelectionWorkflow } from "./useExternalImportSelectionWorkflow";

type ExternalImportSelectionPanelProps = {
  workflow: ExternalImportSelectionWorkflow;
  onViewHistory?: () => void;
  onCloseDialog?: () => void;
};

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function ImportProgress({
  workflow,
  onViewHistory,
  onCloseDialog,
}: {
  workflow: ExternalImportSelectionWorkflow;
  onViewHistory?: () => void;
  onCloseDialog?: () => void;
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
      <>
        <div className="external-import__state is-success" role="status" aria-live="polite">
          <CheckCircle2 size={18} aria-hidden="true" />
          <span>{extCopy.selectionPanel.completedReadingResults}</span>
        </div>
        {/* 完成后的去处引导:保守一行两个 secondary 按钮,不加卡片不重排既有区块。 */}
        {onViewHistory || onCloseDialog ? (
          <div className="external-import__result-actions">
            {onViewHistory ? (
              <button
                type="button"
                className="external-import__button is-secondary"
                onClick={onViewHistory}
              >
                <History size={15} aria-hidden="true" />
                {extCopy.selectionPanel.viewImportHistory}
              </button>
            ) : null}
            {onCloseDialog ? (
              <button
                type="button"
                className="external-import__button is-secondary"
                onClick={onCloseDialog}
              >
                <X size={15} aria-hidden="true" />
                {extCopy.selectionPanel.closeAndReturn}
              </button>
            ) : null}
          </div>
        ) : null}
      </>
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
  onViewHistory,
  onCloseDialog,
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
            <div className="external-import__toolbar-actions">
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
              {/* 已选为 0 时禁用而不是隐藏:按钮位置固定,不会在勾选过程中跳来跳去。 */}
              <button
                type="button"
                className="external-import__button is-secondary"
                disabled={
                  !workflow.selectionEditable ||
                  selectionBusy ||
                  (workflow.selection?.selectedCount ?? 0) === 0
                }
                onClick={workflow.deselectLoaded}
              >
                {workflow.pendingAction === "deselect-loaded" ? (
                  <LoaderCircle className="external-import__spinner" size={15} />
                ) : (
                  <CircleSlash size={15} />
                )}
                {extCopy.selectionPanel.deselectLoaded}
              </button>
            </div>
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

          {/* 只在候选取完后才敢下结论:误选目录的典型场景(如狩技盒子安装根)候选只有
              个位数,首页即完整,引导照常出现;但分页未取完时「本页没有可导入项」不等于
              「整个来源没有可导入项」——第 2 页才出现的 ready 候选会把玩家误导去重选目录。 */}
          {workflow.previewState.candidates.length > 0 &&
          workflow.previewState.nextCursor === null &&
          !workflow.previewState.candidates.some(
            (candidate) =>
              candidate.previewStatus === "ready" ||
              candidate.previewStatus === "name_collision" ||
              candidate.previewStatus === "metadata_invalid",
          ) ? (
            <div className="external-import__state is-muted" role="status" aria-live="polite">
              <CircleAlert size={18} aria-hidden="true" />
              <span>{extCopy.selectionPanel.noImportableHint}</span>
            </div>
          ) : null}

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

      {/* 只留说明文字:开始导入按钮已上移到 Dialog 常驻底栏。
          候选列表可以很长,把主操作留在滚动区意味着玩家必须滚到底才能导入。 */}
      {workflow.selectionEditable && workflow.selection ? (
        <div className="external-import__start-row">
          <div>
            <strong>{extCopy.selectionPanel.importOnlyTitle}</strong>
            <span>{extCopy.selectionPanel.importOnlyDescription}</span>
          </div>
        </div>
      ) : null}

      <ImportProgress
        workflow={workflow}
        onViewHistory={onViewHistory}
        onCloseDialog={onCloseDialog}
      />
      <ExternalImportResultPanel workflow={workflow.result} />
    </section>
  );
}
