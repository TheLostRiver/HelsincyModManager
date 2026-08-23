import type { GameSetupStatus } from "../game-setup/gameSetupTypes";
import type { DashboardCopy } from "./dashboardCopy";

// 设置步骤只保留语义（数量与激活判定）；标题/说明文本来自 dashboardCopy.steps。

export type SetupStepItem = { title: string; meta: string; isActive: boolean };

export function resolveSetupSteps(status: GameSetupStatus, steps: DashboardCopy["steps"]): SetupStepItem[] {
  const activeStepIndex = resolveActiveSetupStepIndex(status);

  return steps.map((step, index) => ({
    ...step,
    isActive: index === activeStepIndex,
  }));
}

export function resolveActiveSetupStepIndex(status: GameSetupStatus) {
  if (status.kind === "configured") {
    return 3;
  }

  if (status.kind === "validating" || status.kind === "invalid") {
    return 1;
  }

  return 0;
}
