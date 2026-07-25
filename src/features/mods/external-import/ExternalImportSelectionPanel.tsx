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
  const state = workflow.importState;
  if (state.status === "idle") {
    return null;
  }
  if (state.status === "starting") {
    return (
      <div className="external-import__state" role="status" aria-live="polite">
        <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
        <span>正在封存选择并创建批量导入任务</span>
      </div>
    );
  }
  if (state.status === "running") {
    const progress =
      state.current !== null && state.total !== null
        ? `（${formatCount(state.current)} / ${formatCount(state.total)}）`
        : "";
    return (
      <div className="external-import__import-progress" role="status" aria-live="polite">
        <div>
          <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
          <span>{getExternalImportPhaseLabel(state.phase)}{progress}</span>
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
          {workflow.cancelPending ? "正在请求取消" : "取消导入"}
        </button>
      </div>
    );
  }
  if (state.status === "cancelling") {
    return (
      <div className="external-import__state is-muted" role="status" aria-live="polite">
        <LoaderCircle className="external-import__spinner" size={18} aria-hidden="true" />
        <span>正在安全取消；等待批量导入专用终态</span>
      </div>
    );
  }
  if (state.status === "completed") {
    return (
      <div className="external-import__state is-success" role="status" aria-live="polite">
        <CheckCircle2 size={18} aria-hidden="true" />
        <span>批量导入已完成。结果明细与重试将在后续结果视图中提供。</span>
      </div>
    );
  }
  if (state.status === "cancelled") {
    return (
      <div className="external-import__state is-muted" role="status" aria-live="polite">
        <XCircle size={18} aria-hidden="true" />
        <span>批量导入已取消；本页面不会根据聚合计数推断部分成功结果。</span>
      </div>
    );
  }
  return (
    <div className="external-import__state is-error" role="alert">
      <CircleAlert size={18} aria-hidden="true" />
      <span>批量导入未完成。请保留当前批次，等待结果视图提供可恢复操作。</span>
    </div>
  );
}

export function ExternalImportSelectionPanel({
  workflow,
}: ExternalImportSelectionPanelProps) {
  const headingId = useId();
  const categories =
    workflow.categoryState.status === "ready" ? workflow.categoryState.options : [];
  const selectionBusy = workflow.pendingAction !== null;

  return (
    <section className="external-import__selection" aria-labelledby={headingId}>
      <header className="external-import__preview-header">
        <div>
          <span className="external-import__eyebrow">候选选择</span>
          <h3 id={headingId}>
            {workflow.selection
              ? `已选择 ${formatCount(workflow.selection.selectedCount)} 项`
              : "正在创建选择快照"}
          </h3>
        </div>
        {workflow.selection?.status === "editing" && workflow.selectionEditable ? (
          <span className="external-import__badge is-neutral">可编辑</span>
        ) : workflow.selection ? (
          <span className="external-import__badge is-warning">
            {workflow.selection.status === "sealed" ? "已封存" : "已过期"}
          </span>
        ) : null}
      </header>

      {workflow.listenerStatus === "failed" ? (
        <div className="external-import__inline-action is-error" role="alert">
          <span>无法监听批量导入进度，启动操作已禁用。</span>
          <button
            type="button"
            className="external-import__button is-secondary"
            onClick={workflow.retryListener}
          >
            <RefreshCcw size={15} />
            重试进度监听
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
            重新加载分类
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
          <span>正在读取选择快照与候选预览</span>
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
            重新加载候选
          </button>
        </div>
      ) : null}

      {workflow.previewState.status === "empty" ? (
        <div className="external-import__empty" role="status" aria-live="polite">
          <FileSearch size={24} aria-hidden="true" />
          <strong>没有可显示的候选</strong>
          <span>扫描共返回 {formatCount(workflow.previewState.totalCount)} 项。</span>
        </div>
      ) : null}

      {workflow.previewState.status === "ready" ? (
        <>
          <div className="external-import__selection-toolbar">
            <span>
              已加载 {formatCount(workflow.previewState.candidates.length)} /{" "}
              {formatCount(workflow.previewState.totalCount)} 项
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
              选择全部可直接导入项
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
              {workflow.previewState.loadingMore ? "正在载入" : "载入更多候选"}
            </button>
          ) : null}
        </>
      ) : null}

      {workflow.selectionEditable && workflow.selection ? (
        <div className="external-import__start-row">
          <div>
            <strong>仅导入到 HMM Mod 库</strong>
            <span>不会安装、启用或写入游戏目录。</span>
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
            开始批量导入
          </button>
        </div>
      ) : null}

      <ImportProgress workflow={workflow} />
    </section>
  );
}
