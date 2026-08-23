import { Map, Settings } from "lucide-react";
import { useGameSetup } from "../../features/game-setup/GameSetupProvider";
import type { GameSetupStatus } from "../../features/game-setup/gameSetupTypes";
import { useActiveProfile } from "../../features/profiles/ActiveProfileProvider";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { appShellCopy, type AppShellCopy } from "../appShellCopy";
import { useTour } from "../onboarding/TourContext";
import { useAppRoute } from "../routing/useAppRoute";
import { ThemeMenu } from "./ThemeMenu";

/** 目录状态未就绪时的占位游戏名。配置完成后改用后端返回的展示名。 */
const FALLBACK_GAME_LABEL = "Monster Hunter: World - Iceborne";

function directoryStatusPill(status: GameSetupStatus, copy: AppShellCopy["header"]) {
  switch (status.kind) {
    case "configured":
      return { tone: "success", label: copy.directoryConfigured } as const;
    case "validating":
      return { tone: "neutral", label: copy.directoryValidating } as const;
    case "invalid":
      return { tone: "danger", label: copy.directoryInvalid } as const;
    case "not_configured":
    default:
      return { tone: "warning", label: copy.directoryNotConfigured } as const;
  }
}

export function AppHeader() {
  const { locale } = useI18n();
  const copy = resolveCopy(appShellCopy, locale).header;
  const { navigate } = useAppRoute();
  const { isTourOpen, startTour } = useTour();
  const { activeProfile } = useActiveProfile();
  const { status: gameSetupStatus } = useGameSetup();
  const activeProfileLabel =
    activeProfile.status === "ready"
      ? activeProfile.profile.name
      : activeProfile.status === "loading"
        ? copy.profileLoading
        : copy.profileUnavailable;
  const activeProfileTone = activeProfile.status === "ready" ? "neutral" : "warning";
  const gameLabel =
    gameSetupStatus.kind === "configured" ? gameSetupStatus.displayName : FALLBACK_GAME_LABEL;
  const directoryStatus = directoryStatusPill(gameSetupStatus, copy);

  return (
    <header className="top-status-bar">
      <div className="current-game">
        <span>{copy.currentGame}</span>
        <strong>{gameLabel}</strong>
      </div>

      <div className="status-actions" aria-label={copy.statusAria}>
        <button
          type="button"
          className="onboarding-launcher"
          aria-label={copy.tourAria}
          title={copy.tourAria}
          disabled={isTourOpen}
          onClick={startTour}
        >
          <Map size={16} aria-hidden="true" />
          <span>{copy.tourLabel}</span>
        </button>
        <span className={`status-pill ${activeProfileTone}`}>
          <span>{copy.profilePill}</span>
          <strong>{activeProfileLabel}</strong>
        </span>
        <span className={`status-pill ${directoryStatus.tone} compact`}>
          <span className={`dot ${directoryStatus.tone}-dot`} aria-hidden="true" />
          <strong>{directoryStatus.label}</strong>
        </span>
        <span className="status-pill neutral compact">
          <span className="dot neutral-dot" aria-hidden="true" />
          <span>{copy.taskIdle}</span>
        </span>
      </div>

      <div className="window-tools" aria-label={copy.windowToolsAria}>
        <ThemeMenu />
        <button
          type="button"
          className="icon-button header-settings-button"
          aria-label={copy.openSettingsAria}
          onClick={() => navigate("/settings")}
        >
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
