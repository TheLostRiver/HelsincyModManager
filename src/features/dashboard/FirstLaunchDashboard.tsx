import {
  FolderOpen,
  LayoutDashboard,
  Search,
} from "lucide-react";

const supportCards = [
  {
    label: "当前支持",
    value: "Monster Hunter: World - Iceborne",
  },
  {
    label: "当前平台",
    value: "Windows",
  },
  {
    label: "Linux / Steam Deck",
    value: "实验性支持预留",
  },
];

const previewCards = [
  { label: "Mod 概览", shortWidth: "80px" },
  { label: "冲突状态", shortWidth: "72px" },
  { label: "前置检查", shortWidth: "76px" },
  { label: "最近备份", shortWidth: "70px" },
];

const setupSteps = [
  {
    title: "扫描 Steam 游戏库",
    meta: "检测已安装游戏和可用候选项。",
    active: true,
  },
  {
    title: "验证游戏目录",
    meta: "确认可执行文件、数据目录和写入权限。",
  },
  {
    title: "创建默认配置档案",
    meta: "在导入前准备一份干净的基线。",
  },
  {
    title: "开始导入模组",
    meta: "仅在目录和配置检查通过后启用。",
  },
];

const setupLogs = [
  { time: "09:42", message: "首次启动设置已打开" },
  { time: "09:42", message: "等待扫描 Steam 游戏库" },
  { time: "--:--", message: "尚未选择游戏目录", muted: true },
];

export function FirstLaunchDashboard() {
  return (
    <>
      <section className="main-workspace" aria-labelledby="workbench-title">
        <header className="main-header">
          <h2 id="workbench-title">工作台</h2>
          <p>首次启动需要先完成游戏目录识别。</p>
        </header>

        <section className="setup-panel" aria-labelledby="setup-title">
          <div className="setup-message">
            <span className="badge warning">
              <span className="dot warning-dot" aria-hidden="true" />
              目录未配置
            </span>
            <h3 id="setup-title">未找到游戏目录</h3>
            <p>需要先识别《怪物猎人：世界 冰原》的安装目录，才能导入和安装 Mod。</p>
          </div>

          <div className="setup-actions">
            <button type="button" className="primary-action">
              <Search size={16} />
              自动扫描 Steam
            </button>
            <button type="button" className="secondary-action">
              <FolderOpen size={16} />
              手动选择游戏目录
            </button>
          </div>

          <div className="support-grid" aria-label="支持信息">
            {supportCards.map((card) => (
              <article className="support-card" key={card.label}>
                <span>{card.label}</span>
                <strong>{card.value}</strong>
              </article>
            ))}
          </div>
        </section>

        <section className="preview-panel" aria-labelledby="preview-title">
          <h3 id="preview-title">完成设置后将显示</h3>
          <p>以下模块会在目录识别、权限校验和默认配置档案创建后启用。</p>

          <div className="preview-heading">
            <LayoutDashboard size={16} />
            <strong>设置完成后启用</strong>
          </div>

          <div className="preview-grid">
            {previewCards.map((card) => (
              <article className="preview-card" key={card.label}>
                <strong>{card.label}</strong>
                <span className="skeleton-line" />
                <span className="skeleton-line short" style={{ width: card.shortWidth }} />
              </article>
            ))}
          </div>
        </section>
      </section>

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
              <p key={`${log.time}-${log.message}`} className={log.muted ? "is-muted" : ""}>
                <time>{log.time}</time>
                {log.message}
              </p>
            ))}
          </div>
        </section>
      </aside>
    </>
  );
}

function StepItem({
  index,
  step,
  isLast,
}: {
  index: number;
  step: { title: string; meta: string; active?: boolean };
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
