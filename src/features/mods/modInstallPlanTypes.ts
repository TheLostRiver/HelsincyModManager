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

export type StartUninstallTaskInput = {
  gameId: GameId;
  modId: string;
  profileId: string;
};

export type GetInstallManifestStatusInput = {
  profileId: string;
  modIds: string[];
};

export type ScanInstallRecoveryInput = {
  gameId: GameId;
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

export type InstallRecoveryStatus = "not_installed" | "completed" | "repair_required" | "unknown";

export type InstallRecoveryIssue =
  | "missing_installed_file_summary"
  | "target_missing"
  | "target_changed"
  | "target_read_failed"
  | "backup_missing"
  | "backup_read_failed";

export type InstallRecoveryIssueSummary = {
  issue: InstallRecoveryIssue;
  count: number;
};

export type InstallRecoverySummary = {
  profileId: string;
  modId: string;
  status: InstallRecoveryStatus;
  managedFileCount: number;
  backupCount: number;
  issueCount: number;
  issues: InstallRecoveryIssueSummary[];
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
