import type { ProfileBackupRetentionDto } from "./profileSaveSettingsTypes";

export const DEFAULT_PROFILE_BACKUP_RETENTION: Readonly<ProfileBackupRetentionDto> = {
  maxCount: 0,
  maxAgeDays: null,
  maxTotalBytes: null,
};
