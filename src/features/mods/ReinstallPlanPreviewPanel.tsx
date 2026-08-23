import { AlertTriangle, CheckCircle2, LoaderCircle, RefreshCw, RotateCcw, X } from "lucide-react";
import { useId, useRef } from "react";
import { useModalFocusTrap } from "../../shared/feedback/useModalFocusTrap";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import {
  canPreviewReinstall,
  getReinstallBlockingReasonLabel,
  getReinstallTaskPhaseLabel,
  type ReinstallTaskState,
} from "./modReinstallTaskState";
import { modLifecycleCopy } from "./modLifecycleCopy";
import { modReinstallCopy, type ModReinstallCopy } from "./modReinstallCopy";
import type { ReinstallPlanPreview } from "./modReinstallTypes";
import {
  getPrerequisiteDecisionCodeLabel,
  getPrerequisiteDecisionMessage,
} from "./modPrerequisiteDecision";
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

function cleanupPendingMessage(
  status: InstallManifestStatus,
  dialog: ModReinstallCopy["dialog"],
) {
  if (status === "committed_cleanup_pending" || status === "cleanup_pending") {
    return dialog.cleanupPending.committed;
  }
  if (status === "rollback_required") {
    return dialog.cleanupPending.rollbackRequired;
  }
  if (status === "repair_required") {
    return dialog.cleanupPending.repairRequired;
  }
  if (status === "unknown") {
    return dialog.cleanupPending.statusUnknown;
  }
  return null;
}

function blockingReasonDetail(code: string, dialog: ModReinstallCopy["dialog"]) {
  switch (code) {
    case "candidate_not_found":
      return dialog.blockingDetails.candidateNotFound;
    case "preview_stale":
      return dialog.blockingDetails.previewStale;
    default:
      return null;
  }
}

function taskStatus(taskState: ReinstallTaskState, copy: ModReinstallCopy) {
  switch (taskState.status) {
    case "starting":
      return { tone: "progress", label: copy.dialog.starting } as const;
    case "running":
      return { tone: "progress", label: getReinstallTaskPhaseLabel(taskState.phase, copy.task) } as const;
    case "completed":
      return { tone: "success", label: copy.dialog.completed } as const;
    case "cancelled":
      return { tone: "neutral", label: copy.dialog.cancelled } as const;
    case "failed":
      return { tone: "danger", label: taskState.message } as const;
    default:
      return null;
  }
}

function PreviewSummary({ preview }: { preview: ReinstallPlanPreview }) {
  const { locale } = useI18n();
  const reCopy = resolveCopy(modReinstallCopy, locale);
  const dialog = reCopy.dialog;
  const prerequisite = resolveCopy(modLifecycleCopy, locale).prerequisite;
  return (
    <section className="reinstall-dialog__preview" aria-label={dialog.summaryAria}>
      <div className="reinstall-dialog__revision-flow">
        <span>
          {preview.installedRevision
            ? dialog.currentRevision(preview.installedRevision.revisionId)
            : dialog.currentRevisionUnknown}
        </span>
        <RefreshCw size={14} aria-hidden="true" />
        <span>
          {preview.candidateRevision
            ? dialog.candidateRevision(preview.candidateRevision.revisionId)
            : dialog.candidateRevisionUnavailable}
        </span>
      </div>

      <dl className="reinstall-dialog__counts">
        <div data-kind="retained">
          <dt>{dialog.countRetained}</dt>
          <dd>{preview.counts.retained}</dd>
        </div>
        <div data-kind="replaced">
          <dt>{dialog.countReplaced}</dt>
          <dd>{preview.counts.replaced}</dd>
        </div>
        <div data-kind="added">
          <dt>{dialog.countAdded}</dt>
          <dd>{preview.counts.added}</dd>
        </div>
        <div data-kind="stale">
          <dt>{dialog.countStale}</dt>
          <dd>{preview.counts.stale}</dd>
        </div>
      </dl>

      {preview.prerequisiteDecision.status !== "ready" ? (
        <div
          className={`reinstall-dialog__notice ${
            preview.prerequisiteDecision.status === "blocked" ? "is-danger" : "is-warning"
          }`}
          role="alert"
        >
          <AlertTriangle size={17} aria-hidden="true" />
          <span>
            {getPrerequisiteDecisionMessage(preview.prerequisiteDecision, prerequisite)}
            {preview.prerequisiteDecision.codes.length > 0
              ? ` ${preview.prerequisiteDecision.codes
                  .map((code) => getPrerequisiteDecisionCodeLabel(code, prerequisite))
                  .join(dialog.codeSeparator)}`
              : ""}
          </span>
        </div>
      ) : null}

      {preview.status === "ready" && preview.prerequisiteDecision.status === "ready" ? (
        <div className="reinstall-dialog__notice is-success" role="status">
          <CheckCircle2 size={17} aria-hidden="true" />
          <span>{dialog.preflightPassed}</span>
        </div>
      ) : null}

      {preview.status === "blocked" ? (
        <div className="reinstall-dialog__blocked" role="alert">
          <div className="reinstall-dialog__notice is-warning">
            <AlertTriangle size={17} aria-hidden="true" />
            <span>{dialog.blockedNotice}</span>
          </div>
          <ul>
            {preview.blockingReasons.map((reason) => {
              const detail = blockingReasonDetail(reason.code, dialog);
              return (
                <li key={reason.code}>
                  <span>{getReinstallBlockingReasonLabel(reason.code, reCopy.task)}</span>
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
  const { locale } = useI18n();
  const reCopy = resolveCopy(modReinstallCopy, locale);
  const dialog = reCopy.dialog;
  const panelRef = useRef<HTMLDivElement | null>(null);
  const titleId = useId();
  const taskActive = taskState.status === "starting" || taskState.status === "running";
  const currentTaskStatus = taskStatus(taskState, reCopy);
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
  const installWarning = cleanupPendingMessage(state.installStatus, dialog);
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
              <h2 id={titleId}>{dialog.title}</h2>
              <p>{state.modName}</p>
            </div>
          </div>
          <button type="button" className="reinstall-dialog__close" onClick={onClose} disabled={taskActive} aria-label={dialog.closeAria}>
            <X size={18} />
          </button>
        </header>

        <div className="reinstall-dialog__body">
          <section className="reinstall-dialog__candidate" aria-labelledby={`${titleId}-candidate`}>
            <div>
              <h3 id={`${titleId}-candidate`}>{dialog.candidateTitle}</h3>
              {state.revisions ? (
                <p>
                  {dialog.revisionOrigin(
                    state.revisions.originRevisionId,
                    state.revisions.displayRevisionId,
                  )}
                </p>
              ) : null}
            </div>
            <div className="reinstall-dialog__candidate-controls">
              <select
                value={state.selectedCandidateRevisionId}
                onChange={(event) => onCandidateChange(event.target.value)}
                disabled={state.catalogStatus !== "ready" || taskActive}
                aria-label={dialog.candidateAria}
              >
                {state.revisions?.revisions.map((revision) => (
                  <option key={revision.revisionId} value={revision.revisionId}>
                    {revision.revisionId}
                  </option>
                ))}
              </select>
              <button type="button" className="reinstall-dialog__button is-secondary" onClick={onPreview} disabled={previewDisabled}>
                <RefreshCw size={15} aria-hidden="true" />
                {dialog.generatePreview}
              </button>
            </div>
          </section>

          {state.catalogStatus === "loading" ? (
            <div className="reinstall-dialog__loading" role="status">
              <LoaderCircle size={17} aria-hidden="true" />
              {dialog.loadingCatalog}
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
              {dialog.loadingPreview}
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
              {dialog.listenerConnecting}
            </div>
          ) : null}
          {listenerStatus === "failed" ? (
            <div className="reinstall-dialog__listener-error" role="alert">
              <span>{dialog.listenerFailed}</span>
              <button type="button" onClick={onRetryListener}>{dialog.retryListener}</button>
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
            {dialog.close}
          </button>
          <button type="button" className="reinstall-dialog__button is-primary" onClick={onConfirm} disabled={!canConfirm}>
            <RotateCcw size={15} aria-hidden="true" />
            {dialog.confirm}
          </button>
        </footer>
      </div>
    </div>
  );
}
