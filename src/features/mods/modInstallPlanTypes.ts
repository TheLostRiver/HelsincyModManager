import type { GameId } from "../game-setup/gameSetupTypes";

export type PreviewImportedModInstallPlanInput = {
  gameId: GameId;
  modId: string;
  layerName: string;
  layerPriority: number;
};

export type StartInstallTaskInput = PreviewImportedModInstallPlanInput & {
  profileId: string;
};

export type InstallPlanProvider = {
  modId: string;
  packageFileId: string;
  layerName: string;
  layerPriority: number;
};

export type InstallPlanAction = InstallPlanProvider & {
  targetPath: string;
};

export type InstallPlanConflict = {
  targetPath: string;
  providers: InstallPlanProvider[];
};

export type InstallPlanPreview = {
  hasBlockingConflicts: boolean;
  actions: InstallPlanAction[];
  conflicts: InstallPlanConflict[];
};
