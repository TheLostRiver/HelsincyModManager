import { useGameSetup } from "../game-setup/useGameSetup";
import { useGamePrerequisites } from "../game-setup/useGamePrerequisites";
import { useGameLaunch } from "../game-launch/useGameLaunch";
import { useInstallRecoveryHealth } from "../install-recovery/useInstallRecoveryHealth";
import { DashboardHeroCard } from "./DashboardHeroCard";
import { DashboardModulePreview } from "./DashboardModulePreview";
import { SetupStatusPanel } from "./SetupStatusPanel";

export function DashboardPage() {
  const gameSetup = useGameSetup("mhw");
  const gamePrerequisites = useGamePrerequisites("mhw");
  const gameLaunch = useGameLaunch("mhw");
  const recoveryHealth = useInstallRecoveryHealth({
    gameId: "mhw",
    enabled: gameSetup.status.kind === "configured",
  });

  /*
   * 启动检测失败不再弹模态。原实现在每次进入工作台时自动弹出「需要配置游戏目录」，
   * 而它提供的标题、文案与两个操作在本页的页头、Hero 卡片和设置状态面板里都已存在——
   * 模态只贡献了阻塞。且 dismiss 只清组件本地 state，离开再回来必然重弹，事实上关不掉。
   * 现在只把模态独有的诊断细节交给设置状态面板常驻展示。
   */
  return (
    <>
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
          prerequisiteState={gamePrerequisites.state}
          onDirectorySelected={gameSetup.saveDirectory}
          onActionError={gameSetup.reportActionError}
          onLaunchGame={gameLaunch.launchGame}
          onRefreshPrerequisites={gamePrerequisites.refresh}
          onScanSteam={gameSetup.scanSteam}
        />
        <DashboardModulePreview />
      </section>

      <SetupStatusPanel
        status={gameSetup.status}
        actionMessage={gameSetup.actionMessage}
        startupDetail={gameSetup.startupNotice?.detail ?? null}
        recoveryHealth={recoveryHealth}
      />
    </>
  );
}
