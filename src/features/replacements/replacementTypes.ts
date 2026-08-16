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
  displayName: string;
  secondaryName?: string;
  aliases: string[];
  internalId: string;
  catalogScope: "production" | "developer_sandbox";
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
