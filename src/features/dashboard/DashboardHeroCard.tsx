import { GameDirectoryActions } from "../game-setup/GameDirectoryActions";
import type { GameSetupStatus } from "../game-setup/gameSetupTypes";
import { supportCards } from "./dashboardData";

type DashboardHeroCardProps = {
  status: GameSetupStatus;
  isBusy: boolean;
  actionMessage: string | null;
  onDirectorySelected: (directory: string) => Promise<void>;
  onActionError: (message: string) => void;
  onScanSteam: () => Promise<void>;
};

export function DashboardHeroCard({
  status,
  isBusy,
  actionMessage,
  onDirectorySelected,
  onActionError,
  onScanSteam,
}: DashboardHeroCardProps) {
  const copy = heroCopyForStatus(status, actionMessage);

  return (
    <section className="setup-panel" aria-labelledby="setup-title">
      <div className="setup-message">
        <span className={`badge ${copy.badgeTone}`}>
          <span className={`dot ${copy.dotClass}`} aria-hidden="true" />
          {copy.badge}
        </span>
        <h3 id="setup-title">{copy.title}</h3>
        <p>{copy.description}</p>
      </div>

      <GameDirectoryActions
        isBusy={isBusy}
        onDirectorySelected={onDirectorySelected}
        onActionError={onActionError}
        onScanSteam={onScanSteam}
      />

      <div className="support-grid" aria-label="支持信息">
        {supportCards.map((card) => (
          <article className="support-card group" key={card.label}>
            <div className="support-card-header">
              <card.icon size={16} color={card.iconColor} strokeWidth={2.1} />
              <span>{card.label}</span>
            </div>
            <strong>{card.value}</strong>
          </article>
        ))}
      </div>
    </section>
  );
}

function heroCopyForStatus(status: GameSetupStatus, actionMessage: string | null) {
  if (status.kind === "configured") {
    return {
      badge: "目录已配置",
      badgeTone: "success",
      dotClass: "success-dot",
      title: status.displayName,
      description: `当前目录：${status.pathLabel}`,
    };
  }

  if (status.kind === "validating") {
    return {
      badge: "正在校验",
      badgeTone: "warning",
      dotClass: "warning-dot",
      title: "正在验证游戏目录",
      description: "Helsincy 正在确认所选目录是否包含 MHW:I 可执行文件。",
    };
  }

  if (status.kind === "invalid") {
    return {
      badge: "校验失败",
      badgeTone: "danger",
      dotClass: "danger-dot",
      title: "目录校验未通过",
      description: status.message || actionMessage || "请选择正确的游戏安装目录。",
    };
  }

  return {
    badge: "目录未配置",
    badgeTone: "warning",
    dotClass: "warning-dot",
    title: "未找到游戏目录",
    description: "需要先识别《怪物猎人：世界 冰原》的安装目录，才能导入和安装 Mod。",
  };
}
