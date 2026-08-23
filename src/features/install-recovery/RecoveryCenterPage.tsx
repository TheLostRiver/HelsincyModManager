import { useMemo, useRef } from "react";
import { AlertTriangle, CircleHelp, FileDown, Loader2, RefreshCw, RotateCcw, ShieldCheck, X } from "lucide-react";
import { useGameSetup } from "../game-setup/GameSetupProvider";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { recoveryCenterCopy, type RecoveryCenterCopy } from "./recoveryCenterCopy";
import {
  deriveRecoveryCenterViewModel,
  type RecoveryCenterIssueView,
  type RecoveryCenterManualAction,
  type RecoveryCenterManualDecision,
  type RecoveryCenterModView,
  type RecoveryCenterRepairSummary,
  type RecoveryCenterViewModel,
} from "./recoveryCenterViewModel";
import {
  useRecoveryDiagnosticsExport,
  type RecoveryDiagnosticsExportState,
} from "./useRecoveryDiagnosticsExport";
import { isManualActionDisabled, resolveManualActionHandler } from "./recoveryCenterManualActions";
import { useRecoveryCenterScan, type RecoveryCenterScanState } from "./useRecoveryCenterScan";
import {
  useRecoveryRollback,
  getRecoveryRollbackPhaseLabel,
  type RecoveryRollbackState,
} from "./useRecoveryRollback";
import type { InstallRecoveryActionPreview } from "../mods/modInstallPlanTypes";

type ActiveRecoveryDiagnosticsExportState = Exclude<RecoveryDiagnosticsExportState, { status: "idle" }>;

export function RecoveryCenterPage() {
  const { locale } = useI18n();
  const copy = resolveCopy(recoveryCenterCopy, locale);
  const gameSetup = useGameSetup();
  const isConfigured = gameSetup.status.kind === "configured";
  const diagnostics = useRecoveryDiagnosticsExport();
  const scan = useRecoveryCenterScan({
    gameId: "mhw",
    enabled: isConfigured,
  });
  const modListRef = useRef<HTMLElement>(null);

  const rollback = useRecoveryRollback({
    gameId: "mhw",
    onCompleted: scan.refresh,
  });

  const scrollToModList = () => {
    modListRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <section className="recovery-center" aria-labelledby="recovery-center-title">
      <header className="recovery-center__hero" data-tour-id="recovery.actions">
        <div className="recovery-center__hero-copy">
          <span className="recovery-center__eyebrow">{copy.page.eyebrow}</span>
          <h2 id="recovery-center-title">{copy.page.title}</h2>
          <p>{copy.page.subtitle}</p>
        </div>
        <div className="recovery-center__hero-actions">
          <button
            type="button"
            className="recovery-center__diagnostics"
            disabled={diagnostics.state.status === "exporting"}
            onClick={diagnostics.requestExport}
          >
            <FileDown size={15} aria-hidden="true" />
            {diagnostics.state.status === "exporting" ? copy.page.exporting : copy.page.exportDiagnostics}
          </button>
          <button
            type="button"
            className="recovery-center__refresh"
            disabled={!isConfigured || scan.state.status === "loading"}
            onClick={scan.refresh}
          >
            <RefreshCw size={15} aria-hidden="true" />
            {copy.page.refresh}
          </button>
        </div>
      </header>

      {diagnostics.state.status !== "idle" ? (
        <DiagnosticExportPanel
          state={diagnostics.state}
          copy={copy}
          onConfirm={diagnostics.confirmExport}
          onCancel={diagnostics.cancelExport}
        />
      ) : null}

      {rollback.state.status !== "idle" ? (
        <RollbackPanel
          state={rollback.state}
          copy={copy}
          onConfirm={rollback.confirmRollback}
          onDismiss={rollback.dismiss}
        />
      ) : null}

      {!isConfigured ? (
        <NotConfiguredPanel copy={copy} />
      ) : (
        <RecoveryCenterBody
          state={scan.state}
          copy={copy}
          onRefresh={scan.refresh}
          onExportDiagnostics={diagnostics.requestExport}
          onScrollToModList={scrollToModList}
          isRefreshing={scan.state.status === "loading"}
          isExporting={diagnostics.state.status === "exporting"}
          rollbackState={rollback.state}
          onRequestRollback={rollback.requestRollback}
          modListRef={modListRef}
        />
      )}
    </section>
  );
}

function RollbackPanel({
  state,
  copy,
  onConfirm,
  onDismiss,
}: {
  state: Exclude<RecoveryRollbackState, { status: "idle" }>;
  copy: RecoveryCenterCopy;
  onConfirm: () => void;
  onDismiss: () => void;
}) {
  const panelCopy = copy.page.rollbackPanel;

  if (state.status === "previewing" || state.status === "starting") {
    return (
      <section className="recovery-center__rollback-panel" role="status" aria-label={panelCopy.statusAria}>
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <Loader2 size={18} className="is-spinning" />
        </div>
        <div className="recovery-center__rollback-body">
          <h3>{state.status === "previewing" ? panelCopy.previewingTitle : panelCopy.startingTitle}</h3>
          <p>{state.modId}</p>
        </div>
      </section>
    );
  }

  if (state.status === "blocked") {
    return (
      <section className="recovery-center__rollback-panel is-failed" aria-labelledby="rollback-blocked-title">
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <AlertTriangle size={18} />
        </div>
        <div className="recovery-center__rollback-body">
          <h3 id="rollback-blocked-title">{panelCopy.blockedTitle}</h3>
          <p>{panelCopy.blockedDetail(state.modId)}</p>
          <BlockReasonList preview={state.preview} blockReasons={copy.blockReasons} />
          <div className="recovery-center__rollback-actions">
            <button type="button" onClick={onDismiss}>
              <X size={14} aria-hidden="true" />
              {panelCopy.close}
            </button>
          </div>
        </div>
      </section>
    );
  }

  if (state.status === "confirming") {
    return (
      <section className="recovery-center__rollback-panel" aria-labelledby="rollback-confirm-title">
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <RotateCcw size={18} />
        </div>
        <div className="recovery-center__rollback-body">
          <h3 id="rollback-confirm-title">{panelCopy.confirmTitle}</h3>
          <p>{panelCopy.confirmBody(state.modId)}</p>
          <RollbackPreviewStats preview={state.preview} panelCopy={panelCopy} />
          <div className="recovery-center__rollback-actions">
            <button type="button" className="is-primary" onClick={onConfirm}>
              <RotateCcw size={14} aria-hidden="true" />
              {panelCopy.confirmAction}
            </button>
            <button type="button" onClick={onDismiss}>
              {panelCopy.cancel}
            </button>
          </div>
        </div>
      </section>
    );
  }

  if (state.status === "running") {
    return (
      <section className="recovery-center__rollback-panel" role="status" aria-label={panelCopy.progressAria}>
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <Loader2 size={18} className="is-spinning" />
        </div>
        <div className="recovery-center__rollback-body">
          <h3>{getRecoveryRollbackPhaseLabel(state.phase, copy.rollback.phases)}</h3>
          <p>{state.modId}</p>
        </div>
      </section>
    );
  }

  if (state.status === "completed") {
    return (
      <section className="recovery-center__rollback-panel is-completed" aria-labelledby="rollback-done-title">
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <ShieldCheck size={18} />
        </div>
        <div className="recovery-center__rollback-body">
          <h3 id="rollback-done-title">{panelCopy.completedTitle}</h3>
          <p>{panelCopy.completedBody(state.modId)}</p>
          <div className="recovery-center__rollback-actions">
            <button type="button" onClick={onDismiss}>
              {panelCopy.close}
            </button>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="recovery-center__rollback-panel is-failed" aria-labelledby="rollback-failed-title">
      <div className="recovery-center__rollback-icon" aria-hidden="true">
        <AlertTriangle size={18} />
      </div>
      <div className="recovery-center__rollback-body">
        <h3 id="rollback-failed-title">{panelCopy.failedTitle}</h3>
        <p>{panelCopy.failedBody(state.modId, rollbackFailureMessage(state, copy))}</p>
        <div className="recovery-center__rollback-actions">
          <button type="button" onClick={onDismiss}>
            {panelCopy.close}
          </button>
        </div>
      </div>
    </section>
  );
}

function rollbackFailureMessage(
  state: Extract<RecoveryRollbackState, { status: "failed" }>,
  copy: RecoveryCenterCopy,
) {
  if (state.reason === "task_failed") {
    return state.backendMessage ?? copy.rollback.failures.taskFallback;
  }
  if (state.reason === "profile_not_ready") return copy.rollback.failures.profileNotReady;
  if (state.reason === "preview_failed") return copy.rollback.failures.previewFailed;
  return copy.rollback.failures.startFailed;
}

function RollbackPreviewStats({
  preview,
  panelCopy,
}: {
  preview: InstallRecoveryActionPreview;
  panelCopy: RecoveryCenterCopy["page"]["rollbackPanel"];
}) {
  return (
    <div className="recovery-center__rollback-stats">
      <span>{panelCopy.statsRemove(preview.removeFileCount)}</span>
      <span>{panelCopy.statsRestore(preview.restoreFileCount)}</span>
      <span>{panelCopy.statsBackups(preview.backupCount)}</span>
    </div>
  );
}

function BlockReasonList({
  preview,
  blockReasons,
}: {
  preview: InstallRecoveryActionPreview;
  blockReasons: RecoveryCenterCopy["blockReasons"];
}) {
  if (preview.blockingReasons.length === 0) {
    return null;
  }

  return (
    <div className="recovery-center__rollback-blocks">
      {preview.blockingReasons.map((reason) => (
        <span key={reason.reason}>
          <strong>{blockReasons[reason.reason] ?? reason.reason} · {reason.count}</strong>
        </span>
      ))}
    </div>
  );
}

function DiagnosticExportPanel({
  state,
  copy,
  onConfirm,
  onCancel,
}: {
  state: ActiveRecoveryDiagnosticsExportState;
  copy: RecoveryCenterCopy;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const diagCopy = copy.page.diagnostics;

  if (state.status === "confirming") {
    return (
      <section
        className="recovery-center__diagnostic-export is-confirming"
        aria-labelledby="diagnostic-export-confirm-title"
      >
        <div>
          <h3 id="diagnostic-export-confirm-title">{diagCopy.confirmTitle}</h3>
          <p>{diagCopy.confirmBody}</p>
        </div>
        <div className="recovery-center__diagnostic-confirmation">
          <ul>
            <li>{diagCopy.bulletContents}</li>
            <li>{diagCopy.bulletPrivacy}</li>
          </ul>
          <div className="recovery-center__diagnostic-export-actions">
            <button type="button" className="is-primary" onClick={onConfirm}>
              <FileDown size={14} aria-hidden="true" />
              {diagCopy.start}
            </button>
            <button type="button" onClick={onCancel}>
              {diagCopy.cancel}
            </button>
          </div>
        </div>
      </section>
    );
  }

  if (state.status === "exporting") {
    return (
      <section className="recovery-center__panel is-loading" role="status" aria-label={diagCopy.statusAria}>
        <div className="recovery-center__state-icon" aria-hidden="true">
          <Loader2 size={18} />
        </div>
        <div>
          <h3>{diagCopy.exportingTitle}</h3>
          <p>{diagCopy.exportingBody}</p>
        </div>
      </section>
    );
  }

  return null;
}

function NotConfiguredPanel({ copy }: { copy: RecoveryCenterCopy }) {
  return (
    <section
      className="recovery-center__panel is-neutral"
      aria-labelledby="recovery-not-configured-title"
      data-tour-id="recovery.state"
    >
      <div className="recovery-center__state-icon" aria-hidden="true">
        <CircleHelp size={18} />
      </div>
      <div data-tour-id="recovery.state-detail">
        <h3 id="recovery-not-configured-title">{copy.page.notConfigured.title}</h3>
        <p>{copy.page.notConfigured.body}</p>
      </div>
    </section>
  );
}

function RecoveryCenterBody({
  state,
  copy,
  onRefresh,
  onExportDiagnostics,
  onScrollToModList,
  isRefreshing,
  isExporting,
  rollbackState,
  onRequestRollback,
  modListRef,
}: {
  state: RecoveryCenterScanState;
  copy: RecoveryCenterCopy;
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
  isRefreshing: boolean;
  isExporting: boolean;
  rollbackState: RecoveryRollbackState;
  onRequestRollback: (modId: string) => void;
  modListRef: React.RefObject<HTMLElement | null>;
}) {
  const summaries = state.status === "ready" ? state.summaries : null;
  const viewModel = useMemo(
    () => (summaries ? deriveRecoveryCenterViewModel(summaries, copy) : null),
    [copy, summaries],
  );

  if (state.status === "idle" || state.status === "loading") {
    return (
      <section
        className="recovery-center__panel is-loading"
        role="status"
        aria-label={copy.page.loading.aria}
        data-tour-id="recovery.state"
      >
        <div className="recovery-center__state-icon" aria-hidden="true">
          <Loader2 size={18} />
        </div>
        <div data-tour-id="recovery.state-detail">
          <h3>{copy.page.loading.title}</h3>
          <p>{copy.page.loading.body}</p>
        </div>
      </section>
    );
  }

  if (state.status === "unavailable") {
    return (
      <section
        className="recovery-center__panel is-unknown"
        aria-labelledby="recovery-unavailable-title"
        data-tour-id="recovery.state"
      >
        <div className="recovery-center__state-icon" aria-hidden="true">
          <CircleHelp size={18} />
        </div>
        <div data-tour-id="recovery.state-detail">
          <h3 id="recovery-unavailable-title">{copy.page.unavailable.title}</h3>
          <p>{copy.page.unavailable.body}</p>
        </div>
      </section>
    );
  }

  if (!viewModel) {
    return null;
  }

  return (
    <RecoveryCenterSummary
      viewModel={viewModel}
      copy={copy}
      onRefresh={onRefresh}
      onExportDiagnostics={onExportDiagnostics}
      onScrollToModList={onScrollToModList}
      isRefreshing={isRefreshing}
      isExporting={isExporting}
      rollbackState={rollbackState}
      onRequestRollback={onRequestRollback}
      modListRef={modListRef}
    />
  );
}

function RecoveryCenterSummary({
  viewModel,
  copy,
  onRefresh,
  onExportDiagnostics,
  onScrollToModList,
  isRefreshing,
  isExporting,
  rollbackState,
  onRequestRollback,
  modListRef,
}: {
  viewModel: RecoveryCenterViewModel;
  copy: RecoveryCenterCopy;
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
  isRefreshing: boolean;
  isExporting: boolean;
  rollbackState: RecoveryRollbackState;
  onRequestRollback: (modId: string) => void;
  modListRef: React.RefObject<HTMLElement | null>;
}) {
  const overview = overviewCopy(viewModel, copy.page.overview);

  return (
    <>
      <section
        className={`recovery-center__panel ${overview.panelClass}`}
        aria-labelledby="recovery-overview-title"
        data-tour-id="recovery.overview"
      >
        <div className="recovery-center__state-icon" aria-hidden="true">
          {overview.icon}
        </div>
        <div className="recovery-center__overview-copy">
          <div className="recovery-center__title-row">
            <h3 id="recovery-overview-title">{overview.title}</h3>
            <span className={`recovery-center__badge is-${viewModel.overview.status}`}>{overview.badge}</span>
          </div>
          <p>{overview.description}</p>
        </div>
      </section>

      <section className="recovery-center__metrics" aria-label={copy.page.metricsAria}>
        <Metric label={copy.page.metricScanned} value={viewModel.overview.scannedModCount} />
        <Metric label={copy.page.metricCompleted} value={viewModel.overview.completedModCount} />
        <Metric label={copy.page.metricAttention} value={viewModel.overview.attentionModCount} />
        <Metric label={copy.page.metricUnknown} value={viewModel.overview.unknownModCount} />
        <Metric label={copy.page.metricManagedFiles} value={viewModel.overview.managedFileCount} />
        <Metric label={copy.page.metricIssues} value={viewModel.overview.issueCount} />
      </section>

      <RepairSummaryPanel summary={viewModel.overview.repairSummary} copy={copy} />

      <ManualHandlingPanel
        manualDecision={viewModel.overview.manualDecision}
        copy={copy}
        onRefresh={onRefresh}
        onExportDiagnostics={onExportDiagnostics}
        onScrollToModList={onScrollToModList}
        isRefreshing={isRefreshing}
        isExporting={isExporting}
      />

      {viewModel.overview.issues.length > 0 ? (
        <IssueList label={copy.page.issuesAggregateAria} issues={viewModel.overview.issues} />
      ) : null}

      <section
        className="recovery-center__mods"
        aria-labelledby="recovery-mod-list-title"
        ref={modListRef}
        data-tour-id="recovery.mods"
      >
        <div className="recovery-center__section-heading">
          <h3 id="recovery-mod-list-title">{copy.page.modsTitle}</h3>
          <span>{copy.page.modsCount(viewModel.mods.length)}</span>
        </div>

        {viewModel.mods.length > 0 ? (
          <div className="recovery-center__mod-list">
            {viewModel.mods.map((mod) => (
              <RecoveryModRow
                key={mod.modId}
                mod={mod}
                copy={copy}
                rollbackState={rollbackState}
                onRequestRollback={onRequestRollback}
              />
            ))}
          </div>
        ) : (
          <article className="recovery-center__empty">
            <ShieldCheck size={18} aria-hidden="true" />
            <p>{copy.page.modEmpty}</p>
          </article>
        )}
      </section>
    </>
  );
}

function ManualHandlingPanel({
  manualDecision,
  copy,
  onRefresh,
  onExportDiagnostics,
  onScrollToModList,
  isRefreshing,
  isExporting,
}: {
  manualDecision: RecoveryCenterManualDecision;
  copy: RecoveryCenterCopy;
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
  isRefreshing: boolean;
  isExporting: boolean;
}) {
  return (
    <section
      className={`recovery-center__manual-decision is-${manualDecision.status}`}
      aria-label={copy.page.manualAria}
      data-tour-id="recovery.manual-actions"
    >
      <div className="recovery-center__manual-copy">
        <h3>{manualDecision.title}</h3>
        <p>{manualDecision.description}</p>
        <strong>{manualDecision.recommendedAction}</strong>
        {manualDecision.safeguards.length > 0 ? (
          <ul>
            {manualDecision.safeguards.map((safeguard) => (
              <li key={safeguard}>{safeguard}</li>
            ))}
          </ul>
        ) : null}
      </div>
      <div className="recovery-center__manual-actions">
        {manualDecision.actions.map((action) => {
          const busyState = { isRefreshing, isExporting };
          const disabled = isManualActionDisabled(action, busyState);
          const handler = resolveManualActionHandler(action, busyState, {
            onRefresh,
            onExportDiagnostics,
            onScrollToModList,
          });

          return (
            <button key={action.id} type="button" disabled={disabled} onClick={handler}>
              {manualActionIcon(action)}
              <span>
                <strong>{action.label}</strong>
                <small>{action.description}</small>
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function manualActionIcon(action: RecoveryCenterManualAction) {
  if (action.id === "export_diagnostics") {
    return <FileDown size={15} aria-hidden="true" />;
  }

  if (action.id === "controlled_recovery") {
    return <RotateCcw size={15} aria-hidden="true" />;
  }

  return <RefreshCw size={15} aria-hidden="true" />;
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <article className="recovery-center__metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function RecoveryModRow({
  mod,
  copy,
  rollbackState,
  onRequestRollback,
}: {
  mod: RecoveryCenterModView;
  copy: RecoveryCenterCopy;
  rollbackState: RecoveryRollbackState;
  onRequestRollback: (modId: string) => void;
}) {
  const isRollbackTarget = mod.status === "rollback_required";
  const isRollbackLocked = rollbackState.status !== "idle";
  const isThisModRollingBack =
    (rollbackState.status === "previewing" ||
      rollbackState.status === "starting" ||
      rollbackState.status === "running") &&
    rollbackState.modId === mod.modId;

  return (
    <article className={`recovery-center__mod is-${mod.statusTone}`}>
      <div className="recovery-center__mod-main">
        <span className={`recovery-center__status is-${mod.statusTone}`}>{mod.statusLabel}</span>
        <strong>{mod.modId}</strong>
      </div>
      <div className="recovery-center__mod-metrics" aria-label={copy.page.modMetricsAria(mod.modId)}>
        {isRollbackTarget ? (
          <button
            type="button"
            className="recovery-center__mod-rollback"
            disabled={isRollbackLocked}
            onClick={() => onRequestRollback(mod.modId)}
          >
            <RotateCcw size={13} aria-hidden="true" />
            {isThisModRollingBack ? copy.page.modRollbackBusy : copy.page.modRollbackAction}
          </button>
        ) : null}
        <span>{copy.page.modFiles(mod.managedFileCount)}</span>
        <span>{copy.page.modBackups(mod.backupCount)}</span>
        <span>{copy.page.modIssues(mod.issueCount)}</span>
      </div>
      {mod.repairSummary.status !== "clear" ? <RepairSummaryPanel summary={mod.repairSummary} copy={copy} compact /> : null}
      {mod.issues.length > 0 ? <IssueList label={copy.page.modIssuesAria(mod.modId)} issues={mod.issues} compact /> : null}
    </article>
  );
}

function RepairSummaryPanel({
  summary,
  copy,
  compact = false,
}: {
  summary: RecoveryCenterRepairSummary;
  copy: RecoveryCenterCopy;
  compact?: boolean;
}) {
  return (
    <section
      className={`recovery-center__repair-summary is-${summary.status} ${compact ? "is-compact" : ""}`}
      aria-label={copy.page.repairAria}
    >
      <div>
        <h4>{summary.title}</h4>
        <p>{summary.description}</p>
      </div>
      <dl>
        <div>
          <dt>{copy.page.repairBlockingReason}</dt>
          <dd>{summary.blockingReason}</dd>
        </div>
        <div>
          <dt>{copy.page.repairNextStep}</dt>
          <dd>{summary.actionLabel}</dd>
        </div>
      </dl>
    </section>
  );
}

function IssueList({
  label,
  issues,
  compact = false,
}: {
  label: string;
  issues: RecoveryCenterIssueView[];
  compact?: boolean;
}) {
  return (
    <div className={`recovery-center__issues ${compact ? "is-compact" : ""}`} aria-label={label}>
      {issues.map((issue) => (
        <span key={issue.issue} className={`is-${issue.severity}`}>
          <strong>
            {issue.label} · {issue.count}
          </strong>
          <small>{issue.guidance}</small>
        </span>
      ))}
    </div>
  );
}

function overviewCopy(viewModel: RecoveryCenterViewModel, copy: RecoveryCenterCopy["page"]["overview"]) {
  if (viewModel.overview.status === "empty") {
    return {
      title: copy.emptyTitle,
      badge: copy.emptyBadge,
      panelClass: "is-neutral",
      description: copy.emptyDescription,
      icon: <ShieldCheck size={18} aria-hidden="true" />,
    };
  }

  if (viewModel.overview.status === "attention") {
    return {
      title: copy.attentionTitle,
      badge: copy.attentionBadge,
      panelClass: "is-attention",
      description:
        viewModel.overview.unknownModCount > 0
          ? copy.attentionDescriptionUnknown
          : copy.attentionDescriptionManual,
      icon: <AlertTriangle size={18} aria-hidden="true" />,
    };
  }

  return {
    title: copy.healthyTitle,
    badge: copy.healthyBadge,
    panelClass: "is-healthy",
    description: copy.healthyDescription(viewModel.overview.completedModCount),
    icon: <ShieldCheck size={18} aria-hidden="true" />,
  };
}
