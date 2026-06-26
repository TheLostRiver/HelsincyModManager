import { AlertTriangle, CircleHelp, Loader2, ShieldCheck } from "lucide-react";
import type { InstallRecoveryIssue } from "../mods/modInstallPlanTypes";
import type { InstallRecoveryHealth, InstallRecoveryHealthStatus } from "../install-recovery/installRecoveryHealth";
import type { InstallRecoveryHealthLoadState } from "../install-recovery/useInstallRecoveryHealth";

type InstallRecoveryHealthPanelProps = {
  state: InstallRecoveryHealthLoadState;
};

const issueLabels: Record<InstallRecoveryIssue, string> = {
  missing_installed_file_summary: "摘要缺失",
  target_missing: "目标缺失",
  target_changed: "目标变更",
  target_read_failed: "读取未知",
  backup_missing: "备份缺失",
  backup_read_failed: "备份未知",
};

export function InstallRecoveryHealthPanel({ state }: InstallRecoveryHealthPanelProps) {
  if (state.status === "idle") {
    return null;
  }

  if (state.status === "loading") {
    return (
      <section className="rail-section recovery-health" aria-labelledby="recovery-health-title">
        <HealthHeader tone="loading" title="安装健康" label="检查中" />
        <div className="recovery-health-card is-loading" role="status">
          <Loader2 size={16} aria-hidden="true" />
          <p>正在读取当前配置档的托管安装摘要。</p>
        </div>
      </section>
    );
  }

  if (state.status === "unavailable") {
    return (
      <section className="rail-section recovery-health" aria-labelledby="recovery-health-title">
        <HealthHeader tone="unknown" title="安装健康" label="状态未知" />
        <div className="recovery-health-card">
          <CircleHelp size={16} aria-hidden="true" />
          <p>无法读取当前配置档的恢复摘要。</p>
        </div>
      </section>
    );
  }

  const copy = healthCopy(state.health);

  return (
    <section className="rail-section recovery-health" aria-labelledby="recovery-health-title">
      <HealthHeader tone={state.health.status} title="安装健康" label={copy.label} />

      <div className={`recovery-health-card ${copy.cardClass}`}>
        {copy.icon}
        <p>{copy.description}</p>
      </div>

      <div className="recovery-health-grid" aria-label="安装恢复摘要">
        <HealthMetric label="扫描" value={`${state.health.scannedModCount}`} />
        <HealthMetric label="需处理" value={`${state.health.attentionModCount}`} />
        <HealthMetric label="未知" value={`${state.health.unknownModCount}`} />
        <HealthMetric label="问题" value={`${state.health.issueCount}`} />
      </div>

      {state.health.issues.length > 0 ? (
        <div className="recovery-issue-list" aria-label="恢复问题聚合">
          {state.health.issues.map((issue) => (
            <span key={issue.issue}>
              {issueLabels[issue.issue]} · {issue.count}
            </span>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function HealthHeader({
  tone,
  title,
  label,
}: {
  tone: InstallRecoveryHealthStatus | "loading" | "unknown";
  title: string;
  label: string;
}) {
  return (
    <div className="section-title-row">
      <h3 id="recovery-health-title">{title}</h3>
      <span className={`recovery-health-badge is-${tone}`}>{label}</span>
    </div>
  );
}

function HealthMetric({ label, value }: { label: string; value: string }) {
  return (
    <article className="recovery-health-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function healthCopy(health: InstallRecoveryHealth) {
  if (health.status === "empty") {
    return {
      label: "无托管记录",
      cardClass: "is-empty",
      description: "当前配置档没有托管安装记录。",
      icon: <ShieldCheck size={16} aria-hidden="true" />,
    };
  }

  if (health.status === "attention") {
    return {
      label: "需要处理",
      cardClass: "is-attention",
      description:
        health.unknownModCount > 0
          ? "存在无法确认的托管安装状态。"
          : "存在需要修复的托管安装状态。",
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  return {
    label: "正常",
    cardClass: "is-healthy",
    description: `${health.completedModCount} 个托管 Mod 状态一致。`,
    icon: <ShieldCheck size={16} aria-hidden="true" />,
  };
}
