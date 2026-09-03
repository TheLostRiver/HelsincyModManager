import { AlertTriangle, CheckCircle2, FileCheck2, Loader2, Trash2, X } from "lucide-react";
import { useRef } from "react";
import {
  DetailSheet,
  Dialog,
  TaskNotice,
  TaskNoticeViewport,
  ToastViewport,
} from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { InstallPlanPreview, InstallRecoveryIssueSummary, UnsafeInstallStatus } from "./modInstallPlanTypes";
import { getManagedInstallTaskPhaseLabel, type ManagedInstallTaskState } from "./modInstallTaskState";
import { modDeleteCopy } from "./modDeleteCopy";
import { modLifecycleCopy, type ModLifecycleCopy } from "./modLifecycleCopy";
import type { ModLifecycleToast } from "./modLifecycleFeedbackState";
import {
  getPrerequisiteDecisionCodeLabel,
  getPrerequisiteDecisionMessage,
} from "./modPrerequisiteDecision";
import "./ModLifecycleFeedback.css";

export type InstallPlanDetailSheetState =
  | { status: "idle" }
  | { status: "loading"; modName: string }
  | { status: "ready"; modName: string; plan: InstallPlanPreview }
  | { status: "error"; modName: string; message: string }
  | {
      status: "recovery-required";
      modName: string;
      recoveryStatus: UnsafeInstallStatus;
      managedFileCount: number;
      backupCount: number;
      issueCount: number;
      issues: InstallRecoveryIssueSummary[];
    };

export type UninstallConfirmationState = {
  modId: string;
  modName: string;
  managedFileCount: number;
  backupCount: number;
  /** Adopted (#286) entries among the managed files; absent when the durable summary did not carry it. */
  adoptedFileCount?: number;
};

type InstallPlanDetailSheetProps = {
  state: InstallPlanDetailSheetState;
  onClose: () => void;
};

function recoveryTitle(status: UnsafeInstallStatus, planSheet: ModLifecycleCopy["planSheet"]) {
  return planSheet.recoveryTitles[status];
}

function sheetTitle(
  state: Exclude<InstallPlanDetailSheetState, { status: "idle" }>,
  planSheet: ModLifecycleCopy["planSheet"],
) {
  if (state.status === "recovery-required") {
    return recoveryTitle(state.recoveryStatus, planSheet);
  }
  if (state.status === "ready" && state.plan.prerequisiteDecision.status === "blocked") {
    return planSheet.prerequisiteBlockedTitle;
  }
  if (state.status === "ready" && state.plan.hasBlockingConflicts) {
    return planSheet.conflictsTitle;
  }
  return planSheet.defaultTitle;
}

export function InstallPlanDetailSheet({ state, onClose }: InstallPlanDetailSheetProps) {
  const { locale } = useI18n();
  const planSheet = resolveCopy(modLifecycleCopy, locale).planSheet;
  if (state.status === "idle") {
    return null;
  }

  const warning =
    state.status === "error" ||
    state.status === "recovery-required" ||
    (state.status === "ready"
      && (state.plan.hasBlockingConflicts || state.plan.prerequisiteDecision.status !== "ready"));
  const icon = state.status === "loading"
    ? <Loader2 className="mod-lifecycle-feedback__spinner" size={20} />
    : warning
      ? <AlertTriangle size={20} />
      : <FileCheck2 size={20} />;

  return (
    <DetailSheet
      open
      title={sheetTitle(state, planSheet)}
      description={state.modName}
      icon={icon}
      onClose={onClose}
      closeLabel={planSheet.closeAria}
    >
      {state.status === "loading" ? (
        <p className="mod-lifecycle-feedback__status" role="status">{planSheet.generating}</p>
      ) : null}
      {state.status === "error" ? (
        <p className="mod-lifecycle-feedback__status is-danger" role="alert">{state.message}</p>
      ) : null}
      {state.status === "recovery-required" ? <RecoveryRequiredSummary state={state} /> : null}
      {state.status === "ready" ? <InstallPlanSummary plan={state.plan} /> : null}
    </DetailSheet>
  );
}

function recoveryStatusMessage(
  status: UnsafeInstallStatus,
  planSheet: ModLifecycleCopy["planSheet"],
) {
  return planSheet.recoveryMessages[status];
}

function RecoveryRequiredSummary({
  state,
}: {
  state: Extract<InstallPlanDetailSheetState, { status: "recovery-required" }>;
}) {
  const { locale } = useI18n();
  const planSheet = resolveCopy(modLifecycleCopy, locale).planSheet;
  return (
    <section className="mod-lifecycle-feedback__section" aria-label={planSheet.recoverySummaryAria}>
      <p className="mod-lifecycle-feedback__status is-danger">{recoveryStatusMessage(state.recoveryStatus, planSheet)}</p>
      <SummaryMetrics
        items={[
          { label: planSheet.metricManagedFiles, value: state.managedFileCount },
          { label: planSheet.metricBackups, value: state.backupCount },
          { label: planSheet.metricChecks, value: state.issueCount, danger: state.issueCount > 0 },
        ]}
      />
      {state.issues.length > 0 ? (
        <ul className="mod-lifecycle-feedback__rows" aria-label={planSheet.recoveryIssuesAria}>
          {state.issues.map((issue) => (
            <li key={issue.issue}>
              <span>{planSheet.recoveryIssueLabels[issue.issue]}</span>
              <strong>{issue.count}</strong>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

function InstallPlanSummary({ plan }: { plan: InstallPlanPreview }) {
  const { locale } = useI18n();
  const lifecycle = resolveCopy(modLifecycleCopy, locale);
  const planSheet = lifecycle.planSheet;
  const previewActions = plan.actions.slice(0, 5);
  const previewConflicts = plan.conflicts.slice(0, 3);
  const prerequisiteDecision = plan.prerequisiteDecision;

  return (
    <section className="mod-lifecycle-feedback__section" aria-label={planSheet.planDetailsAria}>
      <p
        className={[
          "mod-lifecycle-feedback__status",
          prerequisiteDecision.status === "blocked"
            ? "is-danger"
            : prerequisiteDecision.status === "warning"
              ? "is-warning"
              : "",
        ].filter(Boolean).join(" ")}
        role={prerequisiteDecision.status === "ready" ? "status" : "alert"}
      >
        {getPrerequisiteDecisionMessage(prerequisiteDecision, lifecycle.prerequisite)}
      </p>
      {prerequisiteDecision.codes.length > 0 ? (
        <ul className="mod-lifecycle-feedback__rows" aria-label={planSheet.prerequisiteResultsAria}>
          {prerequisiteDecision.codes.map((code) => (
            <li key={code}>
              <span>{getPrerequisiteDecisionCodeLabel(code, lifecycle.prerequisite)}</span>
            </li>
          ))}
        </ul>
      ) : null}
      <SummaryMetrics
        items={[
          { label: planSheet.metricActions, value: plan.actions.length },
          { label: planSheet.metricConflicts, value: plan.conflicts.length, danger: plan.hasBlockingConflicts },
        ]}
      />
      {previewActions.length > 0 ? (
        <div className="mod-lifecycle-feedback__paths" aria-label={planSheet.pathPreviewAria}>
          {previewActions.map((action) => (
            <code key={`${action.modId}:${action.packageFileId}:${action.targetPath}`}>{action.targetPath}</code>
          ))}
        </div>
      ) : (
        <p className="mod-lifecycle-feedback__status">{planSheet.noActions}</p>
      )}
      {previewConflicts.length > 0 ? (
        <div className="mod-lifecycle-feedback__paths is-danger" aria-label={planSheet.conflictPreviewAria}>
          {previewConflicts.map((conflict) => <code key={conflict.targetPath}>{conflict.targetPath}</code>)}
        </div>
      ) : null}
    </section>
  );
}

function SummaryMetrics({
  items,
}: {
  items: Array<{ label: string; value: number; danger?: boolean }>;
}) {
  return (
    <dl className="mod-lifecycle-feedback__metrics">
      {items.map((item) => (
        <div key={item.label} data-danger={item.danger || undefined}>
          <dt>{item.label}</dt>
          <dd>{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

type UninstallConfirmationDialogProps = {
  state: UninstallConfirmationState | null;
  blockerMessage: string | null;
  onCancel: () => void;
  onConfirm: () => void;
};

export function UninstallConfirmationDialog({
  state,
  blockerMessage,
  onCancel,
  onConfirm,
}: UninstallConfirmationDialogProps) {
  const { locale } = useI18n();
  const lifecycle = resolveCopy(modLifecycleCopy, locale);
  const uninstallCopy = lifecycle.uninstallDialog;
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  if (state === null) {
    return null;
  }
  const adoptedFileCount = state.adoptedFileCount ?? 0;
  const metrics = [
    { label: lifecycle.planSheet.metricManagedFiles, value: state.managedFileCount },
    { label: lifecycle.planSheet.metricBackups, value: state.backupCount },
    ...(adoptedFileCount > 0
      ? [{ label: lifecycle.planSheet.metricAdoptedFiles, value: adoptedFileCount }]
      : []),
  ];

  return (
    <Dialog
      open
      title={uninstallCopy.title}
      description={state.modName}
      icon={<AlertTriangle size={20} />}
      onClose={onCancel}
      closeLabel={uninstallCopy.closeAria}
      closeOnBackdrop={false}
      initialFocusRef={cancelButtonRef}
      role="alertdialog"
      footer={
        <>
          <button ref={cancelButtonRef} type="button" className="mod-lifecycle-feedback__button" onClick={onCancel}>
            {uninstallCopy.cancel}
          </button>
          <button
            type="button"
            className="mod-lifecycle-feedback__button is-danger"
            onClick={onConfirm}
            disabled={blockerMessage !== null}
          >
            <Trash2 size={16} aria-hidden="true" />
            {uninstallCopy.confirm}
          </button>
        </>
      }
    >
      <div className="mod-lifecycle-feedback__dialog-copy">
        <p>{uninstallCopy.body}</p>
        <SummaryMetrics items={metrics} />
        {adoptedFileCount > 0 ? (
          <p className="mod-lifecycle-feedback__status is-warning">
            {uninstallCopy.adoptedWarning(adoptedFileCount)}
          </p>
        ) : null}
        {blockerMessage ? (
          <p className="mod-lifecycle-feedback__status is-danger" role="alert">{blockerMessage}</p>
        ) : null}
      </div>
    </Dialog>
  );
}

// Mod deletion (#276): the page projects backend preview facts only. The install
// gate lives in Rust, so an entry may be marked skip with a backend-derived reason.
export type ModDeletionConfirmationEntry = {
  modId: string;
  displayName: string;
  revisionCount: number;
  categoryLabels: string[];
  affectedProfiles: string[];
  skip?: boolean;
  skipReason?: string;
};

export type ModDeletionConfirmation = {
  mods: ModDeletionConfirmationEntry[];
};

type DeleteConfirmationDialogProps = {
  state: ModDeletionConfirmation | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
};

export function DeleteConfirmationDialog({
  state,
  busy,
  onCancel,
  onConfirm,
}: DeleteConfirmationDialogProps) {
  const { locale } = useI18n();
  const deleteCopy = resolveCopy(modDeleteCopy, locale).dialog;
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  if (state === null) {
    return null;
  }

  const pending = state.mods.filter((entry) => entry.skip !== true);
  const batch = state.mods.length > 1;
  const primary = pending[0] ?? state.mods[0] ?? null;

  return (
    <Dialog
      open
      title={batch ? deleteCopy.batchTitle : deleteCopy.singleTitle}
      description={batch ? undefined : primary?.displayName}
      icon={<AlertTriangle size={20} />}
      onClose={onCancel}
      closeLabel={deleteCopy.closeAria}
      closeOnBackdrop={false}
      initialFocusRef={cancelButtonRef}
      role="alertdialog"
      footer={
        <>
          <button
            ref={cancelButtonRef}
            type="button"
            className="mod-lifecycle-feedback__button"
            onClick={onCancel}
          >
            {deleteCopy.cancel}
          </button>
          <button
            type="button"
            className="mod-lifecycle-feedback__button is-danger"
            onClick={onConfirm}
            disabled={busy || pending.length === 0}
          >
            <Trash2 size={16} aria-hidden="true" />
            {busy ? deleteCopy.confirmBusy : deleteCopy.confirm}
          </button>
        </>
      }
    >
      <div className="mod-lifecycle-feedback__dialog-copy">
        {batch ? (
          <>
            <p>{deleteCopy.batchBody}</p>
            <ul className="mod-lifecycle-feedback__delete-list">
              {state.mods.map((entry) => (
                <li key={entry.modId}>
                  <span>{entry.displayName}</span>
                  {entry.skipReason ? <em>{entry.skipReason}</em> : null}
                </li>
              ))}
            </ul>
          </>
        ) : primary === null ? null
          : primary.skip === true ? (
            <p className="mod-lifecycle-feedback__status is-danger" role="alert">
              {primary.skipReason ?? deleteCopy.skipInstalled}
            </p>
          ) : (
            <>
              <p>{deleteCopy.body}</p>
              <SummaryMetrics
                items={[
                  { label: deleteCopy.metricRevisions, value: primary.revisionCount },
                  { label: deleteCopy.metricCategories, value: primary.categoryLabels.length },
                ]}
              />
              {primary.categoryLabels.length > 0 ? (
                <p className="mod-lifecycle-feedback__status">{primary.categoryLabels.join(" / ")}</p>
              ) : null}
              <p className="mod-lifecycle-feedback__status">
                {`${deleteCopy.affectedProfiles}: ${primary.affectedProfiles.length > 0
                  ? primary.affectedProfiles.join(" / ")
                  : deleteCopy.affectedProfilesEmpty}`}
              </p>
            </>
          )}
        <p className="mod-lifecycle-feedback__audit-note">{deleteCopy.retainedAudit}</p>
      </div>
    </Dialog>
  );
}

type ManagedInstallTaskFeedbackProps = {
  taskState: ManagedInstallTaskState;
  toast: ModLifecycleToast | null;
  onDismissToast: () => void;
};

export function ManagedInstallTaskFeedback({
  taskState,
  toast,
  onDismissToast,
}: ManagedInstallTaskFeedbackProps) {
  const { locale } = useI18n();
  const lifecycle = resolveCopy(modLifecycleCopy, locale);
  const taskFeedback = lifecycle.taskFeedback;
  const runningTask = taskState.status === "running" ? taskState : null;

  return (
    <>
      {runningTask ? (
        <TaskNoticeViewport label={taskFeedback.noticeViewportAria}>
          <TaskNotice
            taskId={runningTask.taskId}
            title={runningTask.operation === "uninstall" ? taskFeedback.uninstallingTitle : taskFeedback.installingTitle}
            message={`${runningTask.modName} · ${getManagedInstallTaskPhaseLabel(runningTask.phase, lifecycle.installTask)}`}
            tone="progress"
          >
            <div className="mod-lifecycle-feedback__task-progress" aria-hidden="true">
              <span />
            </div>
          </TaskNotice>
        </TaskNoticeViewport>
      ) : null}

      {toast ? (
        <ToastViewport label={taskFeedback.toastViewportAria}>
          <article className={`mod-lifecycle-feedback__toast is-${toast.tone}`} data-toast-id={toast.id}>
            <span className="mod-lifecycle-feedback__toast-icon" aria-hidden="true">
              {toast.tone === "success" ? <CheckCircle2 size={18} /> : <AlertTriangle size={18} />}
            </span>
            <div>
              <strong>{toast.title}</strong>
              <p>{toast.message}</p>
            </div>
            <button type="button" onClick={onDismissToast} aria-label={taskFeedback.dismissAria} title={taskFeedback.dismissAria}>
              <X size={16} aria-hidden="true" />
            </button>
          </article>
        </ToastViewport>
      ) : null}
    </>
  );
}
