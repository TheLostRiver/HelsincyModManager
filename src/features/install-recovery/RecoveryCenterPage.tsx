import { AlertTriangle, CircleHelp, Loader2, RefreshCw, ShieldCheck } from "lucide-react";
import { useGameSetup } from "../game-setup/useGameSetup";
import type { RecoveryCenterIssueView, RecoveryCenterModView, RecoveryCenterViewModel } from "./recoveryCenterViewModel";
import { useRecoveryCenterScan, type RecoveryCenterScanState } from "./useRecoveryCenterScan";

export function RecoveryCenterPage() {
  const gameSetup = useGameSetup("mhw");
  const isConfigured = gameSetup.status.kind === "configured";
  const scan = useRecoveryCenterScan({
    gameId: "mhw",
    enabled: isConfigured,
  });

  return (
    <section className="recovery-center" aria-labelledby="recovery-center-title">
      <header className="recovery-center__hero">
        <div className="recovery-center__hero-copy">
          <span className="recovery-center__eyebrow">只读恢复扫描</span>
          <h2 id="recovery-center-title">恢复中心</h2>
          <p>查看当前配置档的托管安装健康状态，先定位需要人工处理的条目。</p>
        </div>
        <button
          type="button"
          className="recovery-center__refresh"
          disabled={!isConfigured || scan.state.status === "loading"}
          onClick={scan.refresh}
        >
          <RefreshCw size={15} aria-hidden="true" />
          刷新
        </button>
      </header>

      {!isConfigured ? (
        <NotConfiguredPanel />
      ) : (
        <RecoveryCenterBody state={scan.state} />
      )}
    </section>
  );
}

function NotConfiguredPanel() {
  return (
    <section className="recovery-center__panel is-neutral" aria-labelledby="recovery-not-configured-title">
      <div className="recovery-center__state-icon" aria-hidden="true">
        <CircleHelp size={18} />
      </div>
      <div>
        <h3 id="recovery-not-configured-title">等待游戏目录配置</h3>
        <p>恢复中心需要先有受控游戏实例，才能读取当前配置档的托管安装摘要。</p>
      </div>
    </section>
  );
}

function RecoveryCenterBody({ state }: { state: RecoveryCenterScanState }) {
  if (state.status === "idle" || state.status === "loading") {
    return (
      <section className="recovery-center__panel is-loading" role="status" aria-label="恢复扫描状态">
        <div className="recovery-center__state-icon" aria-hidden="true">
          <Loader2 size={18} />
        </div>
        <div>
          <h3>正在读取恢复摘要</h3>
          <p>正在从后端读取当前配置档的托管安装状态。</p>
        </div>
      </section>
    );
  }

  if (state.status === "unavailable") {
    return (
      <section className="recovery-center__panel is-unknown" aria-labelledby="recovery-unavailable-title">
        <div className="recovery-center__state-icon" aria-hidden="true">
          <CircleHelp size={18} />
        </div>
        <div>
          <h3 id="recovery-unavailable-title">恢复摘要不可用</h3>
          <p>无法确认当前托管安装状态。请稍后刷新，或先回到 Mod 管理页避免继续安装/卸载。</p>
        </div>
      </section>
    );
  }

  return <RecoveryCenterSummary viewModel={state.viewModel} />;
}

function RecoveryCenterSummary({ viewModel }: { viewModel: RecoveryCenterViewModel }) {
  const copy = overviewCopy(viewModel);

  return (
    <>
      <section className={`recovery-center__panel ${copy.panelClass}`} aria-labelledby="recovery-overview-title">
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

      {viewModel.overview.issues.length > 0 ? (
        <IssueList label="恢复问题聚合" issues={viewModel.overview.issues} />
      ) : null}

      <section className="recovery-center__mods" aria-labelledby="recovery-mod-list-title">
        <div className="recovery-center__section-heading">
          <h3 id="recovery-mod-list-title">托管 Mod 状态</h3>
          <span>{viewModel.mods.length} 项</span>
        </div>

        {viewModel.mods.length > 0 ? (
          <div className="recovery-center__mod-list">
            {viewModel.mods.map((mod) => (
              <RecoveryModRow key={mod.modId} mod={mod} />
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

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <article className="recovery-center__metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function RecoveryModRow({ mod }: { mod: RecoveryCenterModView }) {
  return (
    <article className={`recovery-center__mod is-${mod.statusTone}`}>
      <div className="recovery-center__mod-main">
        <span className={`recovery-center__status is-${mod.statusTone}`}>{mod.statusLabel}</span>
        <strong>{mod.modId}</strong>
      </div>
      <div className="recovery-center__mod-metrics" aria-label={`${mod.modId} 恢复摘要`}>
        <span>{mod.managedFileCount} 文件</span>
        <span>{mod.backupCount} 备份</span>
        <span>{mod.issueCount} 问题</span>
      </div>
      {mod.issues.length > 0 ? <IssueList label={`${mod.modId} 恢复问题`} issues={mod.issues} compact /> : null}
    </article>
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
        <span key={issue.issue}>
          {issue.label} · {issue.count}
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
