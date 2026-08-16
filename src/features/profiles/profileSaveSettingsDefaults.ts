import type { ProfileBackupRetentionDto } from "./profileSaveSettingsTypes";

export const DEFAULT_PROFILE_BACKUP_RETENTION: Readonly<ProfileBackupRetentionDto> = {
  maxCount: 50,
  maxAgeDays: 90,
  maxTotalBytes: null,
};
