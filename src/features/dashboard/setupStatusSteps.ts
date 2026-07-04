import type { GameSetupStatus } from "../game-setup/gameSetupTypes";

export const setupSteps = [
  {
    title: "扫描 Steam 游戏库",
    meta: "检测已安装游戏和可用候选项。",
  },
  {
    title: "验证游戏目录",
    meta: "确认可执行文件、数据目录和写入权限。",
  },
  {
    title: "创建默认配置档案",
    meta: "在导入前准备一份干净的基线。",
  },
  {
    title: "开始导入模组",
    meta: "仅在目录和配置检查通过后启用。",
  },
] as const;

export type SetupStep = (typeof setupSteps)[number];
export type SetupStepItem = SetupStep & { isActive: boolean };

export function resolveSetupSteps(status: GameSetupStatus): SetupStepItem[] {
  const activeStepIndex = resolveActiveSetupStepIndex(status);

  return setupSteps.map((step, index) => ({
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
