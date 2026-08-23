import { CheckCircle2, CircleDashed, Play } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { GameDirectoryActions } from "../game-setup/GameDirectoryActions";
import { GameDirectoryCandidateList } from "../game-setup/GameDirectoryCandidateList";
import { GamePrerequisitePanel } from "../game-setup/GamePrerequisitePanel";
import { gameSetupCopy } from "../game-setup/gameSetupCopy";
import { messageForError } from "../game-setup/gameSetupViewModel";
import type { GamePrerequisiteLoadState } from "../game-setup/gamePrerequisiteTypes";
import type { GameDirectoryCandidate, GameSetupStatus } from "../game-setup/gameSetupTypes";
import { gameLaunchCopy } from "../game-launch/gameLaunchCopy";
import { messageForGameLaunchOutcome, type GameLaunchOutcome } from "../game-launch/useGameLaunch";
import type { GameLaunchErrorCode } from "../game-launch/gameLaunchTypes";
import { dashboardCopy, type DashboardCopy } from "./dashboardCopy";
import { supportCards } from "./dashboardData";

type DashboardLaunchState = {
  isLaunchingGame: boolean;
  outcome: GameLaunchOutcome | null;
  errorCode: GameLaunchErrorCode | null;
};

type DashboardHeroCardProps = {
  status: GameSetupStatus;
  candidates: GameDirectoryCandidate[];
  isBusy: boolean;
  actionMessage: string | null;
  launchState: DashboardLaunchState;
  prerequisiteState: GamePrerequisiteLoadState;
  onDirectorySelected: (directory: string) => Promise<void>;
  onActionError: (message: string) => void;
  onLaunchGame: () => Promise<unknown>;
  onRefreshPrerequisites: () => Promise<void>;
  onScanSteam: () => Promise<void>;
};

export function DashboardHeroCard({
  status,
  candidates,
  isBusy,
  actionMessage,
  launchState,
  prerequisiteState,
  onDirectorySelected,
  onActionError,
  onLaunchGame,
  onRefreshPrerequisites,
  onScanSteam,
}: DashboardHeroCardProps) {
  const { locale } = useI18n();
  const heroCopyDict = resolveCopy(dashboardCopy, locale);
  const setupErrors = resolveCopy(gameSetupCopy, locale).errors;
  const launchCopyDict = resolveCopy(gameLaunchCopy, locale);
  const copy = heroCopyForStatus(status, actionMessage, heroCopyDict, setupErrors);
  const isLaunchReady = status.kind === "configured";
  const launchCopy = launchCopyForStatus(status, heroCopyDict.hero.launchStates);
  const launchMessage = messageForGameLaunchOutcome(launchState.outcome, launchState.errorCode, launchCopyDict);
  const LaunchStatusIcon = isLaunchReady ? CheckCircle2 : CircleDashed;

  return (
    <section
      className="setup-panel"
      aria-labelledby="setup-title"
      data-tour-id="dashboard.game-setup"
    >
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

      <div
        className={`launch-action-card${isLaunchReady ? "" : " is-disabled"}`}
        role="group"
        aria-label={heroCopyDict.hero.launchGroupAria}
        data-tour-id="dashboard.launch-game"
      >
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
          <span>{launchState.isLaunchingGame ? heroCopyDict.hero.launching : heroCopyDict.hero.launchButton}</span>
        </button>
        {launchMessage ? (
          <p className="launch-status-note" role="status">
            {launchMessage}
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

      <GamePrerequisitePanel
        state={prerequisiteState}
        onRefresh={onRefreshPrerequisites}
        tourId="dashboard.prerequisites"
      />

      <div className="support-grid" aria-label={heroCopyDict.hero.supportAria}>
        {supportCards.map((card) => (
          <article className="support-card group" key={card.id}>
            <div className="support-card-header">
              <card.icon size={16} color={card.iconColor} strokeWidth={2.1} />
              <span>
                {"labelKey" in card ? heroCopyDict.supportCards[card.labelKey] : card.label}
              </span>
            </div>
            <strong>
              {"valueKey" in card ? heroCopyDict.supportCards[card.valueKey] : card.value}
            </strong>
          </article>
        ))}
      </div>
    </section>
  );
}

function launchCopyForStatus(status: GameSetupStatus, copy: DashboardCopy["hero"]["launchStates"]) {
  if (status.kind === "configured") {
    return {
      status: copy.readyStatus,
      description: copy.readyDescription,
    };
  }

  if (status.kind === "validating") {
    return {
      status: copy.validatingStatus,
      description: copy.validatingDescription,
    };
  }

  if (status.kind === "invalid") {
    return {
      status: copy.invalidStatus,
      description: copy.blockedDescription,
    };
  }

  return {
    status: copy.notConfiguredStatus,
    description: copy.blockedDescription,
  };
}

function heroCopyForStatus(
  status: GameSetupStatus,
  actionMessage: string | null,
  copyDict: DashboardCopy,
  setupErrors: Parameters<typeof messageForError>[1],
) {
  const copy = copyDict.hero.setupStates;

  if (status.kind === "configured") {
    return {
      badge: copy.configuredBadge,
      badgeTone: "success",
      dotClass: "success-dot",
      title: status.displayName,
      description: copy.configuredDescription(status.pathLabel),
    };
  }

  if (status.kind === "validating") {
    return {
      badge: copy.validatingBadge,
      badgeTone: "warning",
      dotClass: "warning-dot",
      title: copy.validatingTitle,
      description: copy.validatingDescription,
    };
  }

  if (status.kind === "invalid") {
    return {
      badge: copy.invalidBadge,
      badgeTone: "danger",
      dotClass: "danger-dot",
      title: copy.invalidTitle,
      description:
        status.backendMessage
        || messageForError(status.errorCode, setupErrors)
        || actionMessage
        || copy.invalidFallbackDescription,
    };
  }

  return {
    badge: copy.notConfiguredBadge,
    badgeTone: "warning",
    dotClass: "warning-dot",
    title: copy.notConfiguredTitle,
    description: copy.notConfiguredDescription,
  };
}
