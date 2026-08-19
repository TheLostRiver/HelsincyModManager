import { Map, Settings } from "lucide-react";
import { useGameSetup } from "../../features/game-setup/GameSetupProvider";
import type { GameSetupStatus } from "../../features/game-setup/gameSetupTypes";
import { useActiveProfile } from "../../features/profiles/ActiveProfileProvider";
import { useTour } from "../onboarding/TourContext";
import { useAppRoute } from "../routing/useAppRoute";
import { ThemeMenu } from "./ThemeMenu";

/** 目录状态未就绪时的占位游戏名。配置完成后改用后端返回的展示名。 */
const FALLBACK_GAME_LABEL = "Monster Hunter: World - Iceborne";

function directoryStatusPill(status: GameSetupStatus) {
  switch (status.kind) {
    case "configured":
      return { tone: "success", label: "目录已配置" } as const;
    case "validating":
      return { tone: "neutral", label: "校验目录中" } as const;
    case "invalid":
      return { tone: "danger", label: "目录不可用" } as const;
    case "not_configured":
    default:
      return { tone: "warning", label: "目录未配置" } as const;
  }
}

export function AppHeader() {
  const { navigate } = useAppRoute();
  const { isTourOpen, startTour } = useTour();
  const { activeProfile } = useActiveProfile();
  const { status: gameSetupStatus } = useGameSetup();
  const activeProfileLabel =
    activeProfile.status === "ready"
      ? activeProfile.profile.name
      : activeProfile.status === "loading"
        ? "读取中"
        : "不可用";
  const activeProfileTone = activeProfile.status === "ready" ? "neutral" : "warning";
  const gameLabel =
    gameSetupStatus.kind === "configured" ? gameSetupStatus.displayName : FALLBACK_GAME_LABEL;
  const directoryStatus = directoryStatusPill(gameSetupStatus);

  return (
    <header className="top-status-bar">
      <div className="current-game">
        <span>当前游戏</span>
        <strong>{gameLabel}</strong>
      </div>

      <div className="status-actions" aria-label="当前状态">
        <button
          type="button"
          className="onboarding-launcher"
          aria-label="打开新手引导"
          title="打开新手引导"
          disabled={isTourOpen}
          onClick={startTour}
        >
          <Map size={16} aria-hidden="true" />
          <span>新手引导</span>
        </button>
        <span className={`status-pill ${activeProfileTone}`}>
          <span>配置档</span>
          <strong>{activeProfileLabel}</strong>
        </span>
        <span className={`status-pill ${directoryStatus.tone} compact`}>
          <span className={`dot ${directoryStatus.tone}-dot`} aria-hidden="true" />
          <strong>{directoryStatus.label}</strong>
        </span>
        <span className="status-pill neutral compact">
          <span className="dot neutral-dot" aria-hidden="true" />
          <span>任务空闲</span>
        </span>
      </div>

      <div className="window-tools" aria-label="窗口工具">
        <ThemeMenu />
        <button
          type="button"
          className="icon-button header-settings-button"
          aria-label="打开设置"
          onClick={() => navigate("/settings")}
        >
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
