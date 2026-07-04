import { useGameSetup } from "../game-setup/useGameSetup";
import { GameSetupFloatingNotice } from "../game-setup/GameSetupFloatingNotice";
import { useGameLaunch } from "../game-launch/useGameLaunch";
import { useInstallRecoveryHealth } from "../install-recovery/useInstallRecoveryHealth";
import { DashboardHeroCard } from "./DashboardHeroCard";
import { DashboardModulePreview } from "./DashboardModulePreview";
import { SetupStatusPanel } from "./SetupStatusPanel";

export function DashboardPage() {
  const gameSetup = useGameSetup("mhw");
  const gameLaunch = useGameLaunch("mhw");
  const recoveryHealth = useInstallRecoveryHealth({
    gameId: "mhw",
    enabled: gameSetup.status.kind === "configured",
  });

  return (
    <>
      <GameSetupFloatingNotice
        notice={gameSetup.startupNotice}
        isBusy={gameSetup.isBusy}
        onRetry={gameSetup.retryStartupDetection}
        onManualSelect={gameSetup.saveDirectory}
        onActionError={gameSetup.reportActionError}
        onDismiss={gameSetup.dismissStartupNotice}
      />

      <section className="main-workspace" aria-labelledby="workbench-title">
        <header className="main-header">
          <h2 id="workbench-title">工作台</h2>
          <p>首次启动需要先完成游戏目录识别。</p>
        </header>

        <DashboardHeroCard
          status={gameSetup.status}
          candidates={gameSetup.candidates}
          isBusy={gameSetup.isBusy}
          actionMessage={gameSetup.actionMessage}
          launchState={{
            isLaunchingGame: gameLaunch.isLaunchingGame,
            message: gameLaunch.gameLaunchMessage,
          }}
          onDirectorySelected={gameSetup.saveDirectory}
          onActionError={gameSetup.reportActionError}
          onLaunchGame={gameLaunch.launchGame}
          onScanSteam={gameSetup.scanSteam}
        />
        <DashboardModulePreview />
      </section>

      <SetupStatusPanel
        status={gameSetup.status}
        actionMessage={gameSetup.actionMessage}
        recoveryHealth={recoveryHealth}
      />
    </>
  );
}
