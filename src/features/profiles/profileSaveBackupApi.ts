import { invoke } from "@tauri-apps/api/core";
import type {
  CheckProfileAutoSaveBackupInput,
  GetSaveBackupBackgroundStatusInput,
  ListProfileSaveBackupsInput,
  ProfileAutoSaveBackupCheckDto,
  SaveBackupBackgroundStatusDto,
  SaveBackupSummaryDto,
  StartProfileSaveBackupInput,
  TaskStartedDto,
} from "./profileSaveBackupTypes";

export function startProfileSaveBackup(input: StartProfileSaveBackupInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_save_backup_task", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      note: input.note,
    },
  });
}

export function listProfileSaveBackups(input: ListProfileSaveBackupsInput): Promise<SaveBackupSummaryDto[]> {
  return invoke<SaveBackupSummaryDto[]>("list_save_backups", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      limit: input.limit,
    },
  });
}

export function checkProfileAutoSaveBackup(input: CheckProfileAutoSaveBackupInput): Promise<ProfileAutoSaveBackupCheckDto> {
  return invoke<ProfileAutoSaveBackupCheckDto>("check_auto_save_backup", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
    },
  });
}

export function getSaveBackupBackgroundStatus(
  input: GetSaveBackupBackgroundStatusInput,
): Promise<SaveBackupBackgroundStatusDto> {
  return invoke<SaveBackupBackgroundStatusDto>("get_save_backup_background_status", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
    },
  });
}
