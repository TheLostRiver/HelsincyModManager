import type { SaveBackupSummaryDto } from "./profileSaveBackupTypes";

export type SaveRestorePreviewDto = {
  backup: SaveBackupSummaryDto;
  fileCount: number;
  totalUncompressedBytes: number;
  preRestoreBackupEnabled: boolean;
  requiresAdditionalConfirmation: boolean;
  warningCodes: string[];
  previewToken: string;
  expiresAt: number;
};

export type SaveRestoreTaskStartedDto = {
  taskId: string;
  kind: "save_restore";
  status: "queued";
};
