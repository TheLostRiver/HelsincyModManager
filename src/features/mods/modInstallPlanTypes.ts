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
  gameId?: GameId;
  profileId: string;
  modIds: string[];
};

export type ScanInstallRecoveryInput = {
  gameId: GameId;
  profileId: string;
  modIds: string[];
};

export type PreviewRecoveryActionInput = {
  gameId: GameId;
  profileId: string;
  modId: string;
  actionKind: InstallRecoveryActionKind;
};

export type StartRecoveryActionTaskInput = PreviewRecoveryActionInput;

export type InstallManifestStatus =
  | "not_installed"
  | "installed"
  | "committed_cleanup_pending"
  | "cleanup_pending"
  | "rollback_required"
  | "repair_required"
  | "unknown";

export type InstallManifestStatusSummary = {
  profileId: string;
  modId: string;
  status: InstallManifestStatus;
  managedFileCount: number;
  backupCount: number;
  /** Exact installed revision from revisioned manifest facts; null for legacy/not-installed. */
  installedRevisionId: string | null;
  /**
   * Entries claimed from an external installation (#286 adopt): no backup, uninstall only
   * deletes them. Both command paths report it; omitted only when the summary source does
   * not carry the fact.
   */
  adoptedFileCount?: number;
};

export type InstallRecoveryStatus =
  | "not_installed"
  | "completed"
  | "committed_cleanup_pending"
  | "cleanup_pending"
  | "rollback_required"
  | "repair_required"
  | "unknown";

export type UnsafeInstallStatus =
  | "committed_cleanup_pending"
  | "cleanup_pending"
  | "rollback_required"
  | "repair_required"
  | "unknown";

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
  adoptedFileCount: number;
  issueCount: number;
  issues: InstallRecoveryIssueSummary[];
};

export type InstallRecoveryActionKind = "rollback_install" | "reconcile_reinstall";

export type InstallRecoveryActionAvailability = "available" | "blocked";

export type InstallRecoveryActionBlockReason =
  | "rollback_state_missing"
  | "missing_installed_file_summary"
  | "target_missing"
  | "target_changed"
  | "target_read_failed"
  | "backup_missing"
  | "backup_read_failed";

export type InstallRecoveryActionBlockReasonSummary = {
  reason: InstallRecoveryActionBlockReason;
  count: number;
};

export type InstallRecoveryActionPreview = {
  profileId: string;
  modId: string;
  actionKind: InstallRecoveryActionKind;
  availability: InstallRecoveryActionAvailability;
  removeFileCount: number;
  restoreFileCount: number;
  backupCount: number;
  blockingIssueCount: number;
  blockingReasons: InstallRecoveryActionBlockReasonSummary[];
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

export type GamePrerequisiteDecisionStatus = "ready" | "warning" | "blocked";

export type GamePrerequisiteDecisionCode =
  | "game_not_configured"
  | "game_directory_invalid"
  | "game_directory_not_writable"
  | "rules_unavailable"
  | "rules_corrupted"
  | "storage_unavailable"
  | "storage_corrupted"
  | "unsupported_game"
  | "missing_required_file"
  | "signature_unverified"
  | "config_read_failed"
  | "config_invalid_json"
  | "config_field_mismatch"
  | "prerequisite_decision_invalid";

export type GamePrerequisiteDecision = {
  status: GamePrerequisiteDecisionStatus;
  rulesVersion: number | null;
  codes: GamePrerequisiteDecisionCode[];
};

export type InstallPlanSummary = {
  hasBlockingConflicts: boolean;
  actions: InstallPlanAction[];
  conflicts: InstallPlanConflict[];
};

export type InstallPlanPreview = InstallPlanSummary & {
  prerequisiteDecision: GamePrerequisiteDecision;
};
