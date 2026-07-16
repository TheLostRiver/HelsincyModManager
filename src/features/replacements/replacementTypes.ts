import type { GameId } from "../game-setup/gameSetupTypes";
import type { TaskStartedDto } from "../mods/modImportTypes";
import type { InstallPlanPreview } from "../mods/modInstallPlanTypes";

export type ListReplacementTargetsInput = {
  gameId: GameId;
  query?: string;
};

export type AnalyzeImportedModReplacementInput = {
  gameId: GameId;
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

export type ReplacementTarget = {
  id: string;
  gameId: string;
  targetType: string;
  displayName: string;
  secondaryName?: string;
  aliases: string[];
  internalId: string;
  metadata: Record<string, unknown>;
};

export type ReplacementSource = {
  id: string;
  sourceType: string;
  internalId: string;
  pathFamily: string;
  supported: boolean;
};

export type ReplacementWarning =
  | "no_supported_assets"
  | "multiple_sources"
  | "unsupported_source"
  | "source_matches_target";

export type ReplacementAnalysis = {
  gameId: string;
  retargetable: boolean;
  matchedAssetCount: number;
  sources: ReplacementSource[];
  warnings: ReplacementWarning[];
};

export type RetargetActionPreview = {
  sourceRelativePath: string;
  targetRelativePath: string;
  sourceInternalId: string;
  targetInternalId: string;
  sourcePathFamily: string;
  targetPathFamily: string;
};

export type InitialRetargetInstallPreview = {
  analysis: ReplacementAnalysis;
  target: ReplacementTarget;
  actions: RetargetActionPreview[];
  warnings: ReplacementWarning[];
  installPlan: InstallPlanPreview;
};

export type RetargetInstallTaskStarted = TaskStartedDto;
