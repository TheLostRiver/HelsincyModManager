import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "../modImportTypes";
import {
  EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE,
  type ExternalImportPreviewPageDto,
  type ExternalImportScanStartedDto,
  type ExternalImportSourceDto,
} from "./externalImportTypes";

export function selectExternalImportSource(): Promise<ExternalImportSourceDto | null> {
  return invoke<ExternalImportSourceDto | null>("select_external_import_source");
}

export function startExternalImportScan(input: { sourceId: string }): Promise<ExternalImportScanStartedDto> {
  return invoke<ExternalImportScanStartedDto>("start_external_import_scan", {
    sourceId: input.sourceId,
  });
}

export function getExternalImportPreview(input: {
  batchId: string;
  cursor?: string | null;
}): Promise<ExternalImportPreviewPageDto> {
  return invoke<ExternalImportPreviewPageDto>("get_external_import_preview", {
    batchId: input.batchId,
    cursor: input.cursor ?? null,
    limit: EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE,
  });
}

export function cancelExternalImportScan(input: { taskId: string }): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", { taskId: input.taskId });
}
