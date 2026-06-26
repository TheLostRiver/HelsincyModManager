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

export type GetInstallManifestStatusInput = {
  profileId: string;
  modIds: string[];
};

export type InstallManifestStatus = "not_installed" | "installed" | "repair_required" | "unknown";

export type InstallManifestStatusSummary = {
  profileId: string;
  modId: string;
  status: InstallManifestStatus;
  managedFileCount: number;
  backupCount: number;
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
