import { AlertTriangle, CheckCircle2, LoaderCircle, RefreshCw, RotateCcw, X } from "lucide-react";
import { useId, useRef } from "react";
import { useModalFocusTrap } from "../../shared/feedback/useModalFocusTrap";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import {
  canPreviewReinstall,
  getReinstallBlockingReasonLabel,
  getReinstallTaskPhaseLabel,
  type ReinstallTaskState,
} from "./modReinstallTaskState";
import type { ReinstallPlanPreview } from "./modReinstallTypes";
import type { ReinstallDialogState } from "./useModReinstallWorkflow";
import "./ReinstallPlanPreviewPanel.css";

type ReinstallPlanPreviewPanelProps = {
  state: ReinstallDialogState;
  taskState: ReinstallTaskState;
  listenerStatus: "loading" | "ready" | "failed";
  canConfirm: boolean;
  onClose: () => void;
  onCandidateChange: (revisionId: string) => void;
  onPreview: () => void;
  onConfirm: () => void;
  onRetryListener: () => void;
};

function cleanupPendingMessage(status: InstallManifestStatus) {
  if (status === "committed_cleanup_pending" || status === "cleanup_pending") {
    return "新版本已提交，但收尾尚未完成。写入操作已暂停，请前往恢复中心完成收敛。";
  }
  if (status === "rollback_required") {
    return "当前重装需要受控恢复，写入操作已暂停。";
  }
  if (status === "repair_required") {
    return "当前安装状态需要人工处理，写入操作已暂停。";
  }
  if (status === "unknown") {
    return "无法确认当前安装状态，写入操作已暂停。";
  }
  return null;
}

function blockingReasonDetail(code: string) {
  switch (code) {
    case "candidate_not_found":
      return "候选版本可能已被移除，请刷新版本列表。";
    case "preview_stale":
      return "重装事实已变化，请重新生成预览。";
    default:
      return null;
  }
}

function taskStatus(taskState: ReinstallTaskState) {
  switch (taskState.status) {
    case "starting":
      return { tone: "progress", label: "正在启动重装任务" } as const;
    case "running":
      return { tone: "progress", label: getReinstallTaskPhaseLabel(taskState.phase) } as const;
    case "completed":
      return { tone: "success", label: "重装完成" } as const;
    case "cancelled":
      return { tone: "neutral", label: "重装已取消" } as const;
    case "failed":
      return { tone: "danger", label: taskState.message } as const;
    default:
      return null;
  }
}

function PreviewSummary({ preview }: { preview: ReinstallPlanPreview }) {
  return (
    <section className="reinstall-dialog__preview" aria-label="重装计划摘要">
      <div className="reinstall-dialog__revision-flow">
        <span>当前 {preview.installedRevision?.revisionId ?? "未知"}</span>
        <RefreshCw size={14} aria-hidden="true" />
        <span>
          候选 {preview.candidateRevision ? preview.candidateRevision.revisionId : "不可用"}
        </span>
      </div>

      <dl className="reinstall-dialog__counts">
        <div data-kind="retained">
          <dt>保留</dt>
          <dd>{preview.counts.retained}</dd>
        </div>
        <div data-kind="replaced">
          <dt>替换</dt>
          <dd>{preview.counts.replaced}</dd>
        </div>
        <div data-kind="added">
          <dt>新增</dt>
          <dd>{preview.counts.added}</dd>
        </div>
        <div data-kind="stale">
          <dt>移除旧项</dt>
          <dd>{preview.counts.stale}</dd>
        </div>
      </dl>

      {preview.status === "ready" ? (
        <div className="reinstall-dialog__notice is-success" role="status">
          <CheckCircle2 size={17} aria-hidden="true" />
          <span>预检通过，可以提交重装。</span>
        </div>
      ) : null}

      {preview.status === "blocked" ? (
        <div className="reinstall-dialog__blocked" role="alert">
          <div className="reinstall-dialog__notice is-warning">
            <AlertTriangle size={17} aria-hidden="true" />
            <span>当前预览存在阻断项。</span>
          </div>
          <ul>
            {preview.blockingReasons.map((reason) => {
              const detail = blockingReasonDetail(reason.code);
              return (
                <li key={reason.code}>
                  <span>{getReinstallBlockingReasonLabel(reason.code)}</span>
                  <strong>{reason.count}</strong>
                  {detail ? <small>{detail}</small> : null}
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
    </section>
  );
}

export function ReinstallPlanPreviewPanel({
  state,
  taskState,
  listenerStatus,
  canConfirm,
  onClose,
  onCandidateChange,
  onPreview,
  onConfirm,
  onRetryListener,
}: ReinstallPlanPreviewPanelProps) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const titleId = useId();
  const taskActive = taskState.status === "starting" || taskState.status === "running";
  const currentTaskStatus = taskStatus(taskState);
  const openModId = state.status === "open" ? state.modId : null;

  useModalFocusTrap({
    active: state.status === "open",
    containerRef: panelRef,
    closeOnEscape: !taskActive,
    onRequestClose: onClose,
    focusKey: openModId,
  });

  if (state.status === "closed") {
    return null;
  }

  const preview = state.previewState.status === "ready" ? state.previewState.preview : null;
  const installWarning = cleanupPendingMessage(state.installStatus);
  const previewDisabled =
    state.catalogStatus !== "ready" ||
    !canPreviewReinstall(state.installStatus, state.selectedCandidateRevisionId, taskState);

  return (
    <div
      className="reinstall-dialog__backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !taskActive) {
          onClose();
        }
      }}
    >
      <div
        ref={panelRef}
        className="reinstall-dialog__panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-busy={taskActive || state.catalogStatus === "loading" || state.previewState.status === "loading"}
        tabIndex={-1}
      >
        <header className="reinstall-dialog__header">
          <div className="reinstall-dialog__heading">
            <span className="reinstall-dialog__icon" aria-hidden="true">
              <RotateCcw size={18} />
            </span>
            <div>
              <h2 id={titleId}>重装 MOD</h2>
              <p>{state.modName}</p>
            </div>
          </div>
          <button type="button" className="reinstall-dialog__close" onClick={onClose} disabled={taskActive} aria-label="关闭">
            <X size={18} />
          </button>
        </header>

        <div className="reinstall-dialog__body">
          <section className="reinstall-dialog__candidate" aria-labelledby={`${titleId}-candidate`}>
            <div>
              <h3 id={`${titleId}-candidate`}>候选版本</h3>
              {state.revisions ? (
                <p>
                  来源版本 {state.revisions.originRevisionId} · 展示版本 {state.revisions.displayRevisionId}
                </p>
              ) : null}
            </div>
            <div className="reinstall-dialog__candidate-controls">
              <select
                value={state.selectedCandidateRevisionId}
                onChange={(event) => onCandidateChange(event.target.value)}
                disabled={state.catalogStatus !== "ready" || taskActive}
                aria-label="候选版本"
              >
                {state.revisions?.revisions.map((revision) => (
                  <option key={revision.revisionId} value={revision.revisionId}>
                    {revision.revisionId}
                  </option>
                ))}
              </select>
              <button type="button" className="reinstall-dialog__button is-secondary" onClick={onPreview} disabled={previewDisabled}>
                <RefreshCw size={15} aria-hidden="true" />
                生成预览
              </button>
            </div>
          </section>

          {state.catalogStatus === "loading" ? (
            <div className="reinstall-dialog__loading" role="status">
              <LoaderCircle size={17} aria-hidden="true" />
              正在读取版本列表
            </div>
          ) : null}
          {state.catalogMessage ? (
            <div className="reinstall-dialog__notice is-warning" role="alert">
              <AlertTriangle size={17} aria-hidden="true" />
              <span>{state.catalogMessage}</span>
            </div>
          ) : null}
          {state.previewState.status === "loading" ? (
            <div className="reinstall-dialog__loading" role="status">
              <LoaderCircle size={17} aria-hidden="true" />
              正在生成安全预览
            </div>
          ) : null}
          {state.previewState.status === "error" ? (
            <div className="reinstall-dialog__notice is-danger" role="alert">
              <AlertTriangle size={17} aria-hidden="true" />
              <span>{state.previewState.message}</span>
            </div>
          ) : null}
          {preview ? <PreviewSummary preview={preview} /> : null}

          {installWarning ? (
            <div className="reinstall-dialog__notice is-danger" role="alert">
              <AlertTriangle size={17} aria-hidden="true" />
              <span>{installWarning}</span>
            </div>
          ) : null}

          {listenerStatus === "loading" ? (
            <div className="reinstall-dialog__loading" role="status">
              <LoaderCircle size={17} aria-hidden="true" />
              正在连接任务状态
            </div>
          ) : null}
          {listenerStatus === "failed" ? (
            <div className="reinstall-dialog__listener-error" role="alert">
              <span>任务状态连接不可用，暂不能提交重装。</span>
              <button type="button" onClick={onRetryListener}>重试连接</button>
            </div>
          ) : null}

          {currentTaskStatus ? (
            <div className={`reinstall-dialog__task is-${currentTaskStatus.tone}`} role={taskState.status === "failed" ? "alert" : "status"}>
              {taskActive ? <LoaderCircle size={17} aria-hidden="true" /> : null}
              <span>{currentTaskStatus.label}</span>
            </div>
          ) : null}
        </div>

        <footer className="reinstall-dialog__footer">
          <button type="button" className="reinstall-dialog__button is-secondary" onClick={onClose} disabled={taskActive}>
            关闭
          </button>
          <button type="button" className="reinstall-dialog__button is-primary" onClick={onConfirm} disabled={!canConfirm}>
            <RotateCcw size={15} aria-hidden="true" />
            确认重装
          </button>
        </footer>
      </div>
    </div>
  );
}
