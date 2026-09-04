import { resolveCopy, useI18n } from "../../shared/i18n";
import { gameSetupCopy, type GameSetupCopy } from "../game-setup/gameSetupCopy";
import { messageForError } from "../game-setup/gameSetupViewModel";
import type {
  GameSetupErrorCode,
  GameSetupStartupNotice,
  GameSetupStatus,
} from "../game-setup/gameSetupTypes";
import type { InstallRecoveryHealthLoadState } from "../install-recovery/useInstallRecoveryHealth";
import { dashboardCopy, type DashboardCopy } from "./dashboardCopy";
import { InstallRecoveryHealthPanel } from "./InstallRecoveryHealthPanel";
import { resolveSetupSteps, type SetupStepItem } from "./setupStatusSteps";

type SetupStatusPanelProps = {
  status: GameSetupStatus;
  /**
   * 上一次手动保存目录的失败原因，与 status 正交：已配置时保存被拒不会
   * 改写 status（#333），失败原因单独存在这里并常驻显示，避免只靠一闪
   * 而过的 toast 传达。
   */
  lastSaveError: GameSetupErrorCode | null;
  actionMessage: string | null;
  /*
   * 启动检测的补充说明。它区分了两种处境完全不同的失败：
   * 「Steam 返回了候选目录但校验未通过」（扫到了、目录不对，该换一个）与
   * 「没有找到可直接保存的 Steam 安装目录」（根本没扫到，该手动选）。
   * 该信息原先只存在于启动时自动弹出的模态里，模态移除后并入这里常驻展示。
   * notice 只带语义 kind，文本在此按当前 locale 取。
   */
  startupNotice: GameSetupStartupNotice | null;
  recoveryHealth: InstallRecoveryHealthLoadState;
};

export function SetupStatusPanel({
  status,
  lastSaveError,
  actionMessage,
  startupNotice,
  recoveryHealth,
}: SetupStatusPanelProps) {
  const { locale } = useI18n();
  const panelCopy = resolveCopy(dashboardCopy, locale);
  const setupErrors = resolveCopy(gameSetupCopy, locale);
  const copy = statusPanelCopy(status, actionMessage, panelCopy, setupErrors.errors);
  const stepItems = resolveSetupSteps(status, panelCopy.steps);
  const startupDetail = deriveStartupDetail(startupNotice, setupErrors);
  /*
   * 已配置时保存失败不影响现状，但得让玩家看见「为什么我选了却没生效」。
   * 它描述的是「上一次操作的结果」，与 startupNotice（启动自动检测的失败）
   * 是两件事，因此各自独立渲染、不互相覆盖。
   */
  const saveErrorDetail =
    status.kind === "configured" && lastSaveError
      ? messageForError(lastSaveError, setupErrors.errors)
      : null;

  return (
    <aside
      className="setup-rail"
      aria-label={panelCopy.setupPanel.railAria}
      data-tour-id="dashboard.setup-status"
    >
      <header className="rail-header">
        <span>{panelCopy.setupPanel.eyebrow}</span>
        <h2>{panelCopy.setupPanel.title}</h2>
        <p>{panelCopy.setupPanel.description}</p>
      </header>

      <section className="rail-card current-state" aria-labelledby="current-state-title">
        <div className="state-title-row">
          <span className={`dot ${copy.dotClass}`} aria-hidden="true" />
          <h3 id="current-state-title">{copy.title}</h3>
        </div>
        <p>{copy.description}</p>
        {startupDetail ? <p className="state-detail">{startupDetail}</p> : null}
        {saveErrorDetail ? (
          <p className="state-detail is-error" role="alert">
            {saveErrorDetail}
          </p>
        ) : null}
        <span className="soft-badge">{copy.badge}</span>
      </section>

      <section className="rail-section" aria-labelledby="next-step-title">
        <div className="section-title-row">
          <h3 id="next-step-title">{panelCopy.setupPanel.nextStepTitle}</h3>
          <span>{copy.stepLabel}</span>
        </div>
        <div className="step-list">
          {stepItems.map((step, index) => (
            <StepItem
              key={step.title}
              index={index + 1}
              step={step}
              isLast={index === stepItems.length - 1}
            />
          ))}
        </div>
      </section>

      <section className="rail-section" aria-labelledby="summary-title">
        <h3 id="summary-title">{panelCopy.setupPanel.summaryTitle}</h3>
        <div className="summary-grid">
          <SummaryBox label={panelCopy.setupPanel.statusLabel} value={copy.summaryStatus} />
          <SummaryBox label={panelCopy.setupPanel.riskLabel} value={copy.summaryRisk} />
        </div>
        <article className="summary-note">
          <strong>{copy.noteTitle}</strong>
          <p>{copy.noteBody}</p>
        </article>
      </section>

      <InstallRecoveryHealthPanel state={recoveryHealth} />

      <section className="rail-section" aria-labelledby="setup-log-title">
        <h3 id="setup-log-title">{panelCopy.setupPanel.logTitle}</h3>
        <div className="log-card">
          {panelCopy.logs.map((log) => (
            <p key={`${log.time}-${log.message}`} className={log.muted ? "is-muted" : ""}>
              <time>{log.time}</time>
              {log.message}
            </p>
          ))}
        </div>
      </section>
    </aside>
  );
}

function deriveStartupDetail(
  notice: GameSetupStartupNotice | null,
  copy: GameSetupCopy,
): string | null {
  if (!notice) {
    return null;
  }

  if (notice.detailKind === "invalid_candidate") {
    return copy.startupNotice.detailInvalidCandidate;
  }
  if (notice.detailKind === "not_found") {
    return copy.startupNotice.detailNotFound;
  }
  if (notice.detailKind === "startup_timeout") {
    return copy.startupNotice.detailStartupTimeout;
  }
  return notice.backendDetail;
}

function statusPanelCopy(
  status: GameSetupStatus,
  actionMessage: string | null,
  copyDict: DashboardCopy,
  setupErrors: GameSetupCopy["errors"],
) {
  const states = copyDict.setupPanel.states;

  if (status.kind === "configured") {
    return {
      title: states.configured.title,
      description: states.configured.description(status.displayName, status.pathLabel),
      badge: states.configured.badge,
      dotClass: "success-dot",
      stepLabel: states.configured.stepLabel,
      summaryStatus: states.configured.summaryStatus,
      summaryRisk: states.configured.summaryRisk,
      noteTitle: states.configured.noteTitle,
      noteBody: states.configured.noteBody,
    };
  }

  if (status.kind === "validating") {
    return {
      title: states.validating.title,
      description: states.validating.description,
      badge: states.validating.badge,
      dotClass: "warning-dot",
      stepLabel: states.validating.stepLabel,
      summaryStatus: states.validating.summaryStatus,
      summaryRisk: states.validating.summaryRisk,
      noteTitle: states.validating.noteTitle,
      noteBody: states.validating.noteBody,
    };
  }

  if (status.kind === "invalid") {
    return {
      title: states.invalid.title,
      description:
        status.backendMessage
        || messageForError(status.errorCode, setupErrors)
        || actionMessage
        || states.invalid.fallbackDescription,
      badge: states.invalid.badge,
      dotClass: "danger-dot",
      stepLabel: states.invalid.stepLabel,
      summaryStatus: states.invalid.summaryStatus,
      summaryRisk: states.invalid.summaryRisk,
      noteTitle: states.invalid.noteTitle,
      noteBody: states.invalid.noteBody,
    };
  }

  return {
    title: states.notConfigured.title,
    description: actionMessage ?? states.notConfigured.defaultDescription,
    badge: states.notConfigured.badge,
    dotClass: "neutral-dot",
    stepLabel: states.notConfigured.stepLabel,
    summaryStatus: states.notConfigured.summaryStatus,
    summaryRisk: states.notConfigured.summaryRisk,
    noteTitle: states.notConfigured.noteTitle,
    noteBody: states.notConfigured.noteBody,
  };
}

function StepItem({
  index,
  step,
  isLast,
}: {
  index: number;
  step: SetupStepItem;
  isLast: boolean;
}) {
  return (
    <article className={`step-item ${step.isActive ? "is-active" : ""}`}>
      <div className="step-rail" aria-hidden="true">
        <span>{index}</span>
        {!isLast && <i />}
      </div>
      <div className="step-body">
        <strong>{step.title}</strong>
        <p>{step.meta}</p>
      </div>
    </article>
  );
}

function SummaryBox({ label, value }: { label: string; value: string }) {
  return (
    <article className="summary-box">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}
