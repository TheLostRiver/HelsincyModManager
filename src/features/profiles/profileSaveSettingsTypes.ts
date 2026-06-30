export type BackupCadence = "manual" | "daily" | "weekly";

export type ProfileDirectoryStatus = "unset" | "valid" | "invalid" | "defaulted";

export type ProfileDirectorySelectionDto = {
  mode: "unset" | "custom" | "default";
  status: ProfileDirectoryStatus;
  pathLabel: string | null;
  messages: string[];
};

export type ProfileBackupScheduleDto = {
  cadence: BackupCadence;
  hour: number | null;
  minute: number | null;
  weekdays: number[];
};

export type ProfileBackupRetentionDto = {
  maxCount: number;
  maxAgeDays: number | null;
};

export type ProfileSaveSettingsDto = {
  profileId: string;
  saveDirectory: ProfileDirectorySelectionDto;
  backupDirectory: ProfileDirectorySelectionDto;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
  updatedAt: number;
};

export type ProfileDirectoryValidationDto = ProfileDirectorySelectionDto;

export type SetProfileSaveSettingsInput = {
  gameId: string;
  profileId: string;
  saveDirectory?: string | null;
  backupDirectory?: string | null;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
};
