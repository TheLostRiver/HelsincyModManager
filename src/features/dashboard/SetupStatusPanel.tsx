import { setupLogs, setupSteps } from "./dashboardData";

export function SetupStatusPanel() {
  return (
    <aside className="setup-rail" aria-label="首次启动设置状态">
      <header className="rail-header">
        <span>首次启动</span>
        <h2>设置状态</h2>
        <p>Helsincy 需要先完成几项检查，才能启用模组管理。</p>
      </header>

      <section className="rail-card current-state" aria-labelledby="current-state-title">
        <div className="state-title-row">
          <span className="dot neutral-dot" aria-hidden="true" />
          <h3 id="current-state-title">等待扫描游戏库</h3>
        </div>
        <p>尚未选择游戏目录。请先在主区域自动扫描 Steam 安装。</p>
        <span className="soft-badge">等待主区扫描</span>
      </section>

      <section className="rail-section" aria-labelledby="next-step-title">
        <div className="section-title-row">
          <h3 id="next-step-title">下一步</h3>
          <span>第 1 / 4 步</span>
        </div>
        <div className="step-list">
          {setupSteps.map((step, index) => (
            <StepItem key={step.title} index={index + 1} step={step} isLast={index === setupSteps.length - 1} />
          ))}
        </div>
      </section>

      <section className="rail-section" aria-labelledby="summary-title">
        <h3 id="summary-title">设置摘要</h3>
        <div className="summary-grid">
          <SummaryBox label="状态" value="未扫描" />
          <SummaryBox label="风险" value="风险：等待检查" />
        </div>
        <article className="summary-note">
          <strong>检查等待中</strong>
          <p>将在设置过程中检查 Steam 访问、游戏文件夹写入权限和配置存储。</p>
        </article>
      </section>

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

function StepItem({
  index,
  step,
  isLast,
}: {
  index: number;
  step: { readonly title: string; readonly meta: string; readonly active?: boolean };
  isLast: boolean;
}) {
  return (
    <article className={`step-item ${step.active ? "is-active" : ""}`}>
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
