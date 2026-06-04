import { useGameSetup } from "../game-setup/useGameSetup";
import { DashboardHeroCard } from "./DashboardHeroCard";
import { DashboardModulePreview } from "./DashboardModulePreview";
import { SetupStatusPanel } from "./SetupStatusPanel";

export function DashboardPage() {
  const gameSetup = useGameSetup("mhw");

  return (
    <>
      <section className="main-workspace" aria-labelledby="workbench-title">
        <header className="main-header">
          <h2 id="workbench-title">工作台</h2>
          <p>首次启动需要先完成游戏目录识别。</p>
        </header>

        <DashboardHeroCard
          status={gameSetup.status}
          isBusy={gameSetup.isBusy}
          actionMessage={gameSetup.actionMessage}
          onDirectorySelected={gameSetup.saveDirectory}
          onScanSteam={gameSetup.scanSteam}
        />
        <DashboardModulePreview />
      </section>

      <SetupStatusPanel status={gameSetup.status} actionMessage={gameSetup.actionMessage} />
    </>
  );
}
