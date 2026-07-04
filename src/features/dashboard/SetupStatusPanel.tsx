import type { GameSetupStatus } from "../game-setup/gameSetupTypes";
import type { InstallRecoveryHealthLoadState } from "../install-recovery/useInstallRecoveryHealth";
import { setupLogs, setupSteps } from "./dashboardData";
import { InstallRecoveryHealthPanel } from "./InstallRecoveryHealthPanel";

type SetupStatusPanelProps = {
  status: GameSetupStatus;
  actionMessage: string | null;
  recoveryHealth: InstallRecoveryHealthLoadState;
};

export function SetupStatusPanel({ status, actionMessage, recoveryHealth }: SetupStatusPanelProps) {
  const copy = statusPanelCopy(status, actionMessage);
  const activeStepIndex = resolveActiveSetupStepIndex(status);

  return (
    <aside className="setup-rail" aria-label="首次启动设置状态">
      <header className="rail-header">
        <span>首次启动</span>
        <h2>设置状态</h2>
        <p>Helsincy 需要先完成几项检查，才能启用模组管理。</p>
      </header>

      <section className="rail-card current-state" aria-labelledby="current-state-title">
        <div className="state-title-row">
          <span className={`dot ${copy.dotClass}`} aria-hidden="true" />
          <h3 id="current-state-title">{copy.title}</h3>
        </div>
        <p>{copy.description}</p>
        <span className="soft-badge">{copy.badge}</span>
      </section>

      <section className="rail-section" aria-labelledby="next-step-title">
        <div className="section-title-row">
          <h3 id="next-step-title">下一步</h3>
          <span>{copy.stepLabel}</span>
        </div>
        <div className="step-list">
          {setupSteps.map((step, index) => (
            <StepItem
              key={step.title}
              index={index + 1}
              step={step}
              isActive={index === activeStepIndex}
              isLast={index === setupSteps.length - 1}
            />
          ))}
        </div>
      </section>

      <section className="rail-section" aria-labelledby="summary-title">
        <h3 id="summary-title">设置摘要</h3>
        <div className="summary-grid">
          <SummaryBox label="状态" value={copy.summaryStatus} />
          <SummaryBox label="风险" value={copy.summaryRisk} />
        </div>
        <article className="summary-note">
          <strong>{copy.noteTitle}</strong>
          <p>{copy.noteBody}</p>
        </article>
      </section>

      <InstallRecoveryHealthPanel state={recoveryHealth} />

      <section className="rail-section" aria-labelledby="setup-log-title">
        <h3 id="setup-log-title">设置日志</h3>
        <div className="log-card">
          {setupLogs.map((log) => (
            <p key={`${log.time}-${log.message}`} className={"muted" in log && log.muted ? "is-muted" : ""}>
              <time>{log.time}</time>
              {log.message}
            </p>
          ))}
        </div>
      </section>
    </aside>
  );
}

function statusPanelCopy(status: GameSetupStatus, actionMessage: string | null) {
  if (status.kind === "configured") {
    return {
      title: "游戏目录已保存",
      description: `已识别 ${status.displayName}，目录摘要：${status.pathLabel}。`,
      badge: "配置完成",
      dotClass: "success-dot",
      stepLabel: "第 4 / 4 步",
      summaryStatus: "已配置",
      summaryRisk: "低：等待 Mod 导入",
      noteTitle: "可以继续",
      noteBody: "游戏目录配置已经保存，后续导入、安装和备份功能会基于该配置继续启用。",
    };
  }

  if (status.kind === "validating") {
    return {
      title: "正在验证目录",
      description: "正在检查所选目录是否包含 MHW:I 可执行文件。",
      badge: "校验中",
      dotClass: "warning-dot",
      stepLabel: "第 2 / 4 步",
      summaryStatus: "校验中",
      summaryRisk: "中：等待结果",
      noteTitle: "正在检查",
      noteBody: "当前只读取玩家主动选择的目录，不会写入游戏目录或读取存档。",
    };
  }

  if (status.kind === "invalid") {
    return {
      title: "目录校验失败",
      description: status.message || actionMessage || "未知错误",
      badge: "需要重新选择",
      dotClass: "danger-dot",
      stepLabel: "第 2 / 4 步",
      summaryStatus: "未通过",
      summaryRisk: "高：目录不可用",
      noteTitle: "检查未通过",
      noteBody: "请选择包含 MonsterHunterWorld.exe 的游戏安装目录。当前失败不会保存为有效配置。",
    };
  }

  return {
    title: "等待选择游戏目录",
    description: actionMessage ?? "尚未选择游戏目录。自动扫描暂未启用时，请先手动选择 MHW:I 安装目录。",
    badge: "等待主区操作",
    dotClass: "neutral-dot",
    stepLabel: "第 1 / 4 步",
    summaryStatus: "未配置",
    summaryRisk: "风险：等待检查",
    noteTitle: "检查等待中",
    noteBody: "将在设置过程中检查游戏可执行文件和配置存储，但不会写入真实游戏目录。",
  };
}

function resolveActiveSetupStepIndex(status: GameSetupStatus) {
  if (status.kind === "configured") {
    return 3;
  }

  if (status.kind === "validating" || status.kind === "invalid") {
    return 1;
  }

  return 0;
}

function StepItem({
  index,
  step,
  isActive,
  isLast,
}: {
  index: number;
  step: { readonly title: string; readonly meta: string };
  isActive: boolean;
  isLast: boolean;
}) {
  return (
    <article className={`step-item ${isActive ? "is-active" : ""}`}>
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
