import type { GameId } from "../game-setup/gameSetupTypes";
import type { TaskStartedDto } from "../mods/modImportTypes";
import type {
  GamePrerequisiteDecision,
  InstallPlanSummary,
} from "../mods/modInstallPlanTypes";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";

export type ListReplacementTargetsInput = {
  gameId: GameId;
  modId: string;
  query?: string;
};

export type AnalyzeImportedModReplacementInput = {
  gameId: GameId;
  profileId: string | null;
  modId: string;
};

export type ListReplacementTargetOccupancyInput = {
  gameId: GameId;
  profileId: string;
  modId: string;
};

/**
 * 跨 Mod 同目标占用的展示投影，只服务于 UI 提示。
 *
 * 前端展示层的 fail-open 是有意设计：清单不可信或读取失败时后端返回空列表，
 * 玩家因此看不到占用提示、按钮也不禁用，但真正的硬门禁仍在预览、任务期计划
 * 构建和 commit 三层（跨 Mod 目标占用会合成阻断冲突），不会放过冲突写入。
 *
 * 类型名刻意不以既有的目标类型名开头：两者前缀相同，会让按"到下一个目标类型
 * 声明为止"切块的契约断言提前收尾（见 replacementApi.test.mjs）。
 */
export type OccupiedReplacementTarget = {
  targetId: string;
  modId: string;
  displayName: string;
};

export type PreviewInitialRetargetInstallInput = {
  gameId: GameId;
  profileId: string;
  modId: string;
  targetId: string;
  layerName: string;
  layerPriority: number;
};

export type StartRetargetInstallTaskInput = PreviewInitialRetargetInstallInput;

export type PreviewRetargetReinstallInput = PreviewInitialRetargetInstallInput;

export type StartRetargetReinstallTaskInput = PreviewRetargetReinstallInput & {
  planToken: string;
};

export type CancelRetargetInstallTaskInput = {
  taskId: string;
};

export type ReplacementTarget = {
  id: string;
  gameId: string;
  targetType: string;
  displayNames: Record<string, string>;
  /** 跨语言压平的检索平表（不带 locale），只供过滤匹配。 */
  aliases: string[];
  /**
   * 按语言分组的别名（locale -> 别名列表），键集 ⊆ displayNames 键集，供展示（#274）。
   * 来源不按语言给别名时（铠甲 catalog）后端省略该键：缺席 = 不知道，不等于空表。
   */
  aliasesByLocale?: Record<string, string[]>;
  internalId: string;
};

export type ReplacementSource = {
  id: string;
  sourceType: string;
  internalId: string;
  supported: boolean;
};

export type ReplacementWarning =
  | "no_supported_assets"
  | "multiple_sources"
  | "unsupported_source"
  | "source_matches_target"
  | "weapon_partial_part_set";

export type ReplacementAnalysis = {
  gameId: string;
  installedTargetId?: string;
  retargetable: boolean;
  matchedAssetCount: number;
  sources: ReplacementSource[];
  warnings: ReplacementWarning[];
};

export type RetargetActionPreview = {
  sourceInternalId: string;
  targetInternalId: string;
};

export type InitialRetargetInstallPreview = {
  analysis: ReplacementAnalysis;
  target: ReplacementTarget;
  actions: RetargetActionPreview[];
  warnings: ReplacementWarning[];
  installPlan: InstallPlanSummary;
  prerequisiteDecision: GamePrerequisiteDecision;
};

export type RetargetInstallTaskStarted = TaskStartedDto;

export type RetargetReinstallPreview = ReinstallPlanPreview;
