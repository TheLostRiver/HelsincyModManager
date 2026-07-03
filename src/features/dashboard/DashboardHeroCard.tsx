import { CheckCircle2, CircleDashed, Play } from "lucide-react";
import { GameDirectoryActions } from "../game-setup/GameDirectoryActions";
import { GameDirectoryCandidateList } from "../game-setup/GameDirectoryCandidateList";
import type { GameDirectoryCandidate, GameSetupStatus } from "../game-setup/gameSetupTypes";
import { supportCards } from "./dashboardData";

type DashboardLaunchState = {
  isLaunchingGame: boolean;
  message: string | null;
};

type DashboardHeroCardProps = {
  status: GameSetupStatus;
  candidates: GameDirectoryCandidate[];
  isBusy: boolean;
  actionMessage: string | null;
  launchState: DashboardLaunchState;
  onDirectorySelected: (directory: string) => Promise<void>;
  onActionError: (message: string) => void;
  onLaunchGame: () => Promise<unknown>;
  onScanSteam: () => Promise<void>;
};

export function DashboardHeroCard({
  status,
  candidates,
  isBusy,
  actionMessage,
  launchState,
  onDirectorySelected,
  onActionError,
  onLaunchGame,
  onScanSteam,
}: DashboardHeroCardProps) {
  const copy = heroCopyForStatus(status, actionMessage);
  const isLaunchReady = status.kind === "configured";
  const launchCopy = launchCopyForStatus(status);
  const LaunchStatusIcon = isLaunchReady ? CheckCircle2 : CircleDashed;

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

      {status.kind !== "configured" ? (
        <GameDirectoryActions
          isBusy={isBusy}
          onDirectorySelected={onDirectorySelected}
          onActionError={onActionError}
          onScanSteam={onScanSteam}
        />
      ) : null}

      <div className={`launch-action-card${isLaunchReady ? "" : " is-disabled"}`} role="group" aria-label="游戏启动">
        <div className={`launch-action-copy${isLaunchReady ? "" : " is-muted"}`}>
          <span>
            <LaunchStatusIcon size={14} aria-hidden="true" />
            {launchCopy.status}
          </span>
          <p>{launchCopy.description}</p>
        </div>
        <button
          type="button"
          className="launch-action-button"
          disabled={!isLaunchReady || launchState.isLaunchingGame}
          aria-busy={launchState.isLaunchingGame ? "true" : undefined}
          onClick={() => {
            if (!isLaunchReady) {
              return;
            }
            void onLaunchGame();
          }}
        >
          <span className="launch-action-button__icon" aria-hidden="true">
            <Play size={17} fill="currentColor" />
          </span>
          <span>{launchState.isLaunchingGame ? "正在启动" : "启动游戏"}</span>
        </button>
        {launchState.message ? (
          <p className="launch-status-note" role="status">
            {launchState.message}
          </p>
        ) : null}
      </div>

      {status.kind !== "configured" ? (
        <GameDirectoryCandidateList
          candidates={candidates}
          isBusy={isBusy}
          onCandidateSelected={onDirectorySelected}
        />
      ) : null}

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

function launchCopyForStatus(status: GameSetupStatus) {
  if (status.kind === "configured") {
    return {
      status: "已准备就绪",
      description: "当前配置档可用，游戏目录已通过校验。",
    };
  }

  if (status.kind === "validating") {
    return {
      status: "等待目录校验",
      description: "目录校验完成后即可启动。",
    };
  }

  if (status.kind === "invalid") {
    return {
      status: "需要重新选择目录",
      description: "配置游戏目录后即可启动。",
    };
  }

  return {
    status: "等待目录配置",
    description: "配置游戏目录后即可启动。",
  };
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
