import { invoke } from "@tauri-apps/api/core";
import type {
  QuerySaveBackupCenterInput,
  SaveBackupCenterPageDto,
  SaveBackupRetentionReportDto,
} from "./backupCenterTypes";

export function querySaveBackupCenter(
  input: QuerySaveBackupCenterInput,
): Promise<SaveBackupCenterPageDto> {
  return invoke<SaveBackupCenterPageDto>("query_save_backup_center", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      trigger: input.trigger,
      status: input.status,
      search: input.search,
      offset: input.offset,
      limit: input.limit,
    },
  });
}

export function updateSaveBackupNote(input: {
  gameId: string;
  profileId: string;
  backupId: string;
  note: string | null;
}): Promise<{ note: string | null }> {
  return invoke<{ note: string | null }>("update_save_backup_note", {
    request: input,
  });
}

export function runSaveBackupRetention(input: {
  gameId: string;
  profileId: string;
}): Promise<SaveBackupRetentionReportDto> {
  return invoke<SaveBackupRetentionReportDto>("run_save_backup_retention", {
    request: input,
  });
}
