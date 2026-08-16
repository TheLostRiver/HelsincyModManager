import { Map, Settings } from "lucide-react";
import { useActiveProfile } from "../../features/profiles/ActiveProfileProvider";
import { useTour } from "../onboarding/TourContext";
import { useAppRoute } from "../routing/useAppRoute";
import { ThemeMenu } from "./ThemeMenu";

export function AppHeader() {
  const { navigate } = useAppRoute();
  const { isTourOpen, startTour } = useTour();
  const { activeProfile } = useActiveProfile();
  const activeProfileLabel =
    activeProfile.status === "ready"
      ? activeProfile.profile.name
      : activeProfile.status === "loading"
        ? "读取中"
        : "不可用";
  const activeProfileTone = activeProfile.status === "ready" ? "neutral" : "warning";

  return (
    <header className="top-status-bar">
      <div className="current-game">
        <span>当前游戏</span>
        <strong>Monster Hunter: World - Iceborne</strong>
      </div>

      <div className="status-actions" aria-label="当前状态">
        <span className={`status-pill ${activeProfileTone}`}>
          <span>配置档</span>
          <strong>{activeProfileLabel}</strong>
        </span>
        <span className="status-pill warning compact">
          <span className="dot warning-dot" aria-hidden="true" />
          <strong>目录未配置</strong>
        </span>
        <span className="status-pill neutral compact">
          <span className="dot neutral-dot" aria-hidden="true" />
          <span>任务空闲</span>
        </span>
      </div>

      <div className="window-tools" aria-label="窗口工具">
        <button
          type="button"
          className="icon-button onboarding-launcher"
          aria-label="打开新手引导"
          title="打开新手引导"
          disabled={isTourOpen}
          onClick={startTour}
        >
          <Map size={16} />
        </button>
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
