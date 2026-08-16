import type { SaveBackupSummaryDto } from "../profiles/profileSaveBackupTypes";
import type {
  ProfileBackupRetentionDto,
  SteamAccountDisplaySummaryDto,
} from "../profiles/profileSaveSettingsTypes";

export type BackupCenterTrigger = SaveBackupSummaryDto["trigger"];
export type BackupCenterStatus = SaveBackupSummaryDto["status"];

export type SaveBackupCenterSummaryDto = {
  backupCount: number;
  archiveBytes: number;
  protectedCount: number;
  attentionCount: number;
};

export type SaveBackupCenterProfileSummaryDto = {
  profileId: string;
  profileName: string;
  isActive: boolean;
  steamAccount: SteamAccountDisplaySummaryDto | null;
  retention: ProfileBackupRetentionDto;
  backupCount: number;
  archiveBytes: number;
  protectedCount: number;
  attentionCount: number;
  budgetSatisfied: boolean;
};

export type SaveBackupCenterItemDto = {
  profileName: string;
  backup: SaveBackupSummaryDto;
};

export type SaveBackupCenterPageDto = {
  offset: number;
  limit: number;
  totalCount: number;
  summary: SaveBackupCenterSummaryDto;
  profiles: SaveBackupCenterProfileSummaryDto[];
  items: SaveBackupCenterItemDto[];
};

export type QuerySaveBackupCenterInput = {
  gameId: string;
  profileId?: string | null;
  trigger?: BackupCenterTrigger | null;
  status?: BackupCenterStatus | null;
  search?: string | null;
  offset: number;
  limit: number;
};

export type SaveBackupRetentionOutcome =
  | "within_policy"
  | "completed"
  | "partial"
  | "blocked"
  | "failed";

export type SaveBackupRetentionReportDto = {
  outcome: SaveBackupRetentionOutcome;
  evidenceDegraded: boolean;
  scannedCount: number;
  protectedCount: number;
  problemCount: number;
  candidateCount: number;
  deletedCount: number;
  partialCount: number;
  blockedCount: number;
  archiveBytesBefore: number;
  archiveBytesAfter: number;
  releasedBytes: number;
  maxTotalBytes: number | null;
  budgetSatisfied: boolean;
};

export type BackupMaintenanceState =
  | { status: "idle" }
  | { status: "running"; elapsedMs: number; startedAt: number }
  | { status: "completed"; elapsedMs: number; report: SaveBackupRetentionReportDto }
  | { status: "error"; elapsedMs: number; message: string };
