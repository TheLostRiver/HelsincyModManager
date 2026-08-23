import { AlertTriangle, CircleHelp, Loader2, ShieldCheck } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { InstallRecoveryHealth, InstallRecoveryHealthStatus } from "../install-recovery/installRecoveryHealth";
import { recoveryCenterCopy } from "../install-recovery/recoveryCenterCopy";
import type { InstallRecoveryHealthLoadState } from "../install-recovery/useInstallRecoveryHealth";
import { dashboardCopy, type DashboardCopy } from "./dashboardCopy";

type InstallRecoveryHealthPanelProps = {
  state: InstallRecoveryHealthLoadState;
};

export function InstallRecoveryHealthPanel({ state }: InstallRecoveryHealthPanelProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(dashboardCopy, locale).recoveryHealth;
  // issue 标签与恢复中心共用同一张表，避免两处映射漂移。
  const issueCopy = resolveCopy(recoveryCenterCopy, locale).issues;

  if (state.status === "idle") {
    return null;
  }

  if (state.status === "loading") {
    return (
      <section className="rail-section recovery-health" aria-labelledby="recovery-health-title">
        <HealthHeader tone="loading" title={copy.title} label={copy.loadingBadge} />
        <div className="recovery-health-card is-loading" role="status">
          <Loader2 size={16} aria-hidden="true" />
          <p>{copy.loadingBody}</p>
        </div>
      </section>
    );
  }

  if (state.status === "unavailable") {
    return (
      <section className="rail-section recovery-health" aria-labelledby="recovery-health-title">
        <HealthHeader tone="unknown" title={copy.title} label={copy.unknownBadge} />
        <div className="recovery-health-card">
          <CircleHelp size={16} aria-hidden="true" />
          <p>{copy.unavailableBody}</p>
        </div>
      </section>
    );
  }

  const healthView = healthCopy(state.health, copy);

  return (
    <section className="rail-section recovery-health" aria-labelledby="recovery-health-title">
      <HealthHeader tone={state.health.status} title={copy.title} label={healthView.label} />

      <div className={`recovery-health-card ${healthView.cardClass}`}>
        {healthView.icon}
        <p>{healthView.description}</p>
      </div>

      <div className="recovery-health-grid" aria-label={copy.metricsAria}>
        <HealthMetric label={copy.metricScanned} value={`${state.health.scannedModCount}`} />
        <HealthMetric label={copy.metricAttention} value={`${state.health.attentionModCount}`} />
        <HealthMetric label={copy.metricUnknown} value={`${state.health.unknownModCount}`} />
        <HealthMetric label={copy.metricIssues} value={`${state.health.issueCount}`} />
      </div>

      {state.health.issues.length > 0 ? (
        <div className="recovery-issue-list" aria-label={copy.issuesAria}>
          {state.health.issues.map((issue) => (
            <span key={issue.issue}>
              {issueCopy[issue.issue].label} · {issue.count}
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

function healthCopy(health: InstallRecoveryHealth, copy: DashboardCopy["recoveryHealth"]) {
  if (health.status === "empty") {
    return {
      label: copy.emptyBadge,
      cardClass: "is-empty",
      description: copy.emptyDescription,
      icon: <ShieldCheck size={16} aria-hidden="true" />,
    };
  }

  if (health.status === "attention") {
    return {
      label: copy.attentionBadge,
      cardClass: "is-attention",
      description:
        health.unknownModCount > 0
          ? copy.attentionDescriptionUnknown
          : copy.attentionDescriptionRepair,
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  return {
    label: copy.healthyBadge,
    cardClass: "is-healthy",
    description: copy.healthyDescription(health.completedModCount),
    icon: <ShieldCheck size={16} aria-hidden="true" />,
  };
}
