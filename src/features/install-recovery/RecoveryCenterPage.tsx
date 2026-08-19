import { useRef } from "react";
import { AlertTriangle, CircleHelp, FileDown, Loader2, RefreshCw, RotateCcw, ShieldCheck, X } from "lucide-react";
import { useGameSetup } from "../game-setup/GameSetupProvider";
import type {
  RecoveryCenterIssueView,
  RecoveryCenterManualAction,
  RecoveryCenterManualDecision,
  RecoveryCenterModView,
  RecoveryCenterRepairSummary,
  RecoveryCenterViewModel,
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

const blockReasonLabels: Record<string, string> = {
  rollback_state_missing: "回滚状态缺失",
  missing_installed_file_summary: "摘要缺失",
  target_missing: "目标缺失",
  target_changed: "目标变更",
  target_read_failed: "目标读取失败",
  backup_missing: "备份缺失",
  backup_read_failed: "备份读取失败",
};

export function RecoveryCenterPage() {
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
          <span className="recovery-center__eyebrow">受控恢复中心</span>
          <h2 id="recovery-center-title">恢复中心</h2>
          <p>查看当前配置档的托管安装健康状态，先定位需要人工处理的条目。</p>
        </div>
        <div className="recovery-center__hero-actions">
          <button
            type="button"
            className="recovery-center__diagnostics"
            disabled={diagnostics.state.status === "exporting"}
            onClick={diagnostics.requestExport}
          >
            <FileDown size={15} aria-hidden="true" />
            {diagnostics.state.status === "exporting" ? "导出中" : "导出诊断"}
          </button>
          <button
            type="button"
            className="recovery-center__refresh"
            disabled={!isConfigured || scan.state.status === "loading"}
            onClick={scan.refresh}
          >
            <RefreshCw size={15} aria-hidden="true" />
            刷新
          </button>
        </div>
      </header>

      {diagnostics.state.status !== "idle" ? (
        <DiagnosticExportPanel
          state={diagnostics.state}
          onConfirm={diagnostics.confirmExport}
          onCancel={diagnostics.cancelExport}
        />
      ) : null}

      {rollback.state.status !== "idle" ? (
        <RollbackPanel
          state={rollback.state}
          onConfirm={rollback.confirmRollback}
          onDismiss={rollback.dismiss}
        />
      ) : null}

      {!isConfigured ? (
        <NotConfiguredPanel />
      ) : (
        <RecoveryCenterBody
          state={scan.state}
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
  onConfirm,
  onDismiss,
}: {
  state: Exclude<RecoveryRollbackState, { status: "idle" }>;
  onConfirm: () => void;
  onDismiss: () => void;
}) {
  if (state.status === "previewing" || state.status === "starting") {
    return (
      <section className="recovery-center__rollback-panel" role="status" aria-label="回滚状态">
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <Loader2 size={18} className="is-spinning" />
        </div>
        <div className="recovery-center__rollback-body">
          <h3>{state.status === "previewing" ? "正在检查回滚条件" : "正在启动回滚任务"}</h3>
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
          <h3 id="rollback-blocked-title">受控回滚不可执行</h3>
          <p>{state.modId} — 后端预检发现阻断条件，当前无法安全回滚。</p>
          <BlockReasonList preview={state.preview} />
          <div className="recovery-center__rollback-actions">
            <button type="button" onClick={onDismiss}>
              <X size={14} aria-hidden="true" />
              关闭
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
          <h3 id="rollback-confirm-title">确认受控回滚</h3>
          <p>将对 {state.modId} 执行受控回滚，恢复到安装前状态。</p>
          <RollbackPreviewStats preview={state.preview} />
          <div className="recovery-center__rollback-actions">
            <button type="button" className="is-primary" onClick={onConfirm}>
              <RotateCcw size={14} aria-hidden="true" />
              确认回滚
            </button>
            <button type="button" onClick={onDismiss}>
              取消
            </button>
          </div>
        </div>
      </section>
    );
  }

  if (state.status === "running") {
    return (
      <section className="recovery-center__rollback-panel" role="status" aria-label="回滚进度">
        <div className="recovery-center__rollback-icon" aria-hidden="true">
          <Loader2 size={18} className="is-spinning" />
        </div>
        <div className="recovery-center__rollback-body">
          <h3>{getRecoveryRollbackPhaseLabel(state.phase)}</h3>
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
          <h3 id="rollback-done-title">回滚完成</h3>
          <p>{state.modId} 已恢复到安装前状态。已触发重新扫描。</p>
          <div className="recovery-center__rollback-actions">
            <button type="button" onClick={onDismiss}>
              关闭
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
        <h3 id="rollback-failed-title">回滚失败</h3>
        <p>{state.modId} — {state.message}</p>
        <div className="recovery-center__rollback-actions">
          <button type="button" onClick={onDismiss}>
            关闭
          </button>
        </div>
      </div>
    </section>
  );
}

function RollbackPreviewStats({ preview }: { preview: InstallRecoveryActionPreview }) {
  return (
    <div className="recovery-center__rollback-stats">
      <span>将删除 {preview.removeFileCount} 个文件</span>
      <span>将恢复 {preview.restoreFileCount} 个文件</span>
      <span>涉及 {preview.backupCount} 个备份</span>
    </div>
  );
}

function BlockReasonList({ preview }: { preview: InstallRecoveryActionPreview }) {
  if (preview.blockingReasons.length === 0) {
    return null;
  }

  return (
    <div className="recovery-center__rollback-blocks">
      {preview.blockingReasons.map((reason) => (
        <span key={reason.reason}>
          <strong>{blockReasonLabels[reason.reason] ?? reason.reason} · {reason.count}</strong>
        </span>
      ))}
    </div>
  );
}

function DiagnosticExportPanel({
  state,
  onConfirm,
  onCancel,
}: {
  state: ActiveRecoveryDiagnosticsExportState;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  if (state.status === "confirming") {
    return (
      <section
        className="recovery-center__diagnostic-export is-confirming"
        aria-labelledby="diagnostic-export-confirm-title"
      >
        <div>
          <h3 id="diagnostic-export-confirm-title">确认导出诊断包</h3>
          <p>导出包会由后端生成已脱敏的支持材料，页面只显示安全摘要。</p>
        </div>
        <div className="recovery-center__diagnostic-confirmation">
          <ul>
            <li>包含平台摘要、已校验 App 日志、已校验任务日志和已校验审计事件。</li>
            <li>页面不展示日志正文、审计正文、本地路径或第三方 Mod 内容。</li>
          </ul>
          <div className="recovery-center__diagnostic-export-actions">
            <button type="button" className="is-primary" onClick={onConfirm}>
              <FileDown size={14} aria-hidden="true" />
              开始导出
            </button>
            <button type="button" onClick={onCancel}>
              取消
            </button>
          </div>
        </div>
      </section>
    );
  }

  if (state.status === "exporting") {
    return (
      <section className="recovery-center__panel is-loading" role="status" aria-label="诊断导出状态">
        <div className="recovery-center__state-icon" aria-hidden="true">
          <Loader2 size={18} />
        </div>
        <div>
          <h3>正在导出诊断包</h3>
          <p>正在生成已脱敏的支持诊断摘要。</p>
        </div>
      </section>
    );
  }

  return null;
}

function NotConfiguredPanel() {
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
        <h3 id="recovery-not-configured-title">等待游戏目录配置</h3>
        <p>恢复中心需要先有受控游戏实例，才能读取当前配置档的托管安装摘要。</p>
      </div>
    </section>
  );
}

function RecoveryCenterBody({
  state,
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
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
  isRefreshing: boolean;
  isExporting: boolean;
  rollbackState: RecoveryRollbackState;
  onRequestRollback: (modId: string) => void;
  modListRef: React.RefObject<HTMLElement | null>;
}) {
  if (state.status === "idle" || state.status === "loading") {
    return (
      <section
        className="recovery-center__panel is-loading"
        role="status"
        aria-label="恢复扫描状态"
        data-tour-id="recovery.state"
      >
        <div className="recovery-center__state-icon" aria-hidden="true">
          <Loader2 size={18} />
        </div>
        <div data-tour-id="recovery.state-detail">
          <h3>正在读取恢复摘要</h3>
          <p>正在从后端读取当前配置档的托管安装状态。</p>
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
          <h3 id="recovery-unavailable-title">恢复摘要不可用</h3>
          <p>无法确认当前托管安装状态。请稍后刷新，或先回到 Mod 管理页避免继续安装/卸载。</p>
        </div>
      </section>
    );
  }

  return (
    <RecoveryCenterSummary
      viewModel={state.viewModel}
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
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
  isRefreshing: boolean;
  isExporting: boolean;
  rollbackState: RecoveryRollbackState;
  onRequestRollback: (modId: string) => void;
  modListRef: React.RefObject<HTMLElement | null>;
}) {
  const copy = overviewCopy(viewModel);

  return (
    <>
      <section
        className={`recovery-center__panel ${copy.panelClass}`}
        aria-labelledby="recovery-overview-title"
        data-tour-id="recovery.overview"
      >
        <div className="recovery-center__state-icon" aria-hidden="true">
          {copy.icon}
        </div>
        <div className="recovery-center__overview-copy">
          <div className="recovery-center__title-row">
            <h3 id="recovery-overview-title">{copy.title}</h3>
            <span className={`recovery-center__badge is-${viewModel.overview.status}`}>{copy.badge}</span>
          </div>
          <p>{copy.description}</p>
        </div>
      </section>

      <section className="recovery-center__metrics" aria-label="恢复扫描聚合摘要">
        <Metric label="扫描 Mod" value={viewModel.overview.scannedModCount} />
        <Metric label="状态正常" value={viewModel.overview.completedModCount} />
        <Metric label="需处理" value={viewModel.overview.attentionModCount} />
        <Metric label="未知" value={viewModel.overview.unknownModCount} />
        <Metric label="托管文件" value={viewModel.overview.managedFileCount} />
        <Metric label="问题" value={viewModel.overview.issueCount} />
      </section>

      <RepairSummaryPanel summary={viewModel.overview.repairSummary} />

      <ManualHandlingPanel
        manualDecision={viewModel.overview.manualDecision}
        onRefresh={onRefresh}
        onExportDiagnostics={onExportDiagnostics}
        onScrollToModList={onScrollToModList}
        isRefreshing={isRefreshing}
        isExporting={isExporting}
      />

      {viewModel.overview.issues.length > 0 ? (
        <IssueList label="恢复问题聚合" issues={viewModel.overview.issues} />
      ) : null}

      <section
        className="recovery-center__mods"
        aria-labelledby="recovery-mod-list-title"
        ref={modListRef}
        data-tour-id="recovery.mods"
      >
        <div className="recovery-center__section-heading">
          <h3 id="recovery-mod-list-title">托管 Mod 状态</h3>
          <span>{viewModel.mods.length} 项</span>
        </div>

        {viewModel.mods.length > 0 ? (
          <div className="recovery-center__mod-list">
            {viewModel.mods.map((mod) => (
              <RecoveryModRow
                key={mod.modId}
                mod={mod}
                rollbackState={rollbackState}
                onRequestRollback={onRequestRollback}
              />
            ))}
          </div>
        ) : (
          <article className="recovery-center__empty">
            <ShieldCheck size={18} aria-hidden="true" />
            <p>当前配置档没有托管安装记录。</p>
          </article>
        )}
      </section>
    </>
  );
}

function ManualHandlingPanel({
  manualDecision,
  onRefresh,
  onExportDiagnostics,
  onScrollToModList,
  isRefreshing,
  isExporting,
}: {
  manualDecision: RecoveryCenterManualDecision;
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
  isRefreshing: boolean;
  isExporting: boolean;
}) {
  return (
    <section
      className={`recovery-center__manual-decision is-${manualDecision.status}`}
      aria-label="人工处理决策"
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
  rollbackState,
  onRequestRollback,
}: {
  mod: RecoveryCenterModView;
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
      <div className="recovery-center__mod-metrics" aria-label={`${mod.modId} 恢复摘要`}>
        {isRollbackTarget ? (
          <button
            type="button"
            className="recovery-center__mod-rollback"
            disabled={isRollbackLocked}
            onClick={() => onRequestRollback(mod.modId)}
          >
            <RotateCcw size={13} aria-hidden="true" />
            {isThisModRollingBack ? "处理中" : "回滚"}
          </button>
        ) : null}
        <span>{mod.managedFileCount} 文件</span>
        <span>{mod.backupCount} 备份</span>
        <span>{mod.issueCount} 问题</span>
      </div>
      {mod.repairSummary.status !== "clear" ? <RepairSummaryPanel summary={mod.repairSummary} compact /> : null}
      {mod.issues.length > 0 ? <IssueList label={`${mod.modId} 恢复问题`} issues={mod.issues} compact /> : null}
    </article>
  );
}

function RepairSummaryPanel({
  summary,
  compact = false,
}: {
  summary: RecoveryCenterRepairSummary;
  compact?: boolean;
}) {
  return (
    <section
      className={`recovery-center__repair-summary is-${summary.status} ${compact ? "is-compact" : ""}`}
      aria-label="恢复处理摘要"
    >
      <div>
        <h4>{summary.title}</h4>
        <p>{summary.description}</p>
      </div>
      <dl>
        <div>
          <dt>阻断原因</dt>
          <dd>{summary.blockingReason}</dd>
        </div>
        <div>
          <dt>下一步</dt>
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

function overviewCopy(viewModel: RecoveryCenterViewModel) {
  if (viewModel.overview.status === "empty") {
    return {
      title: "没有托管安装记录",
      badge: "空记录",
      panelClass: "is-neutral",
      description: "当前配置档还没有由 Helsincy 托管的安装项。",
      icon: <ShieldCheck size={18} aria-hidden="true" />,
    };
  }

  if (viewModel.overview.status === "attention") {
    return {
      title: "发现需要关注的安装状态",
      badge: "需要处理",
      panelClass: "is-attention",
      description:
        viewModel.overview.unknownModCount > 0
          ? "部分托管安装状态无法确认，恢复中心会先阻断自动处理动作。"
          : "部分托管安装状态需要人工处理，自动安装/卸载入口应保持阻断。",
      icon: <AlertTriangle size={18} aria-hidden="true" />,
    };
  }

  return {
    title: "托管安装状态正常",
    badge: "正常",
    panelClass: "is-healthy",
    description: `${viewModel.overview.completedModCount} 个托管 Mod 与 manifest 摘要一致。`,
    icon: <ShieldCheck size={18} aria-hidden="true" />,
  };
}
