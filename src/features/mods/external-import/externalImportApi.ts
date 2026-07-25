import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "../modImportTypes";
import {
  EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE,
  type ExternalImportBatchStartedDto,
  type ExternalImportPreviewPageDto,
  type ExternalImportScanStartedDto,
  type ExternalImportSelectionDto,
  type ExternalImportSelectionMutationInputDto,
  type ExternalImportSelectionMutationResultDto,
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
  selectionId?: string | null;
  cursor?: string | null;
}): Promise<ExternalImportPreviewPageDto> {
  return invoke<ExternalImportPreviewPageDto>("get_external_import_preview", {
    batchId: input.batchId,
    selectionId: input.selectionId ?? null,
    cursor: input.cursor ?? null,
    limit: EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE,
  });
}

export function createExternalImportSelection(input: {
  batchId: string;
}): Promise<ExternalImportSelectionDto> {
  return invoke<ExternalImportSelectionDto>("create_external_import_selection", {
    batchId: input.batchId,
  });
}

export function updateExternalImportSelection(input: {
  selectionId: string;
  expectedRevision: number;
  entries: ExternalImportSelectionMutationInputDto[];
}): Promise<ExternalImportSelectionMutationResultDto> {
  return invoke<ExternalImportSelectionMutationResultDto>(
    "update_external_import_selection",
    input,
  );
}

export function selectAllExternalImportCandidates(input: {
  selectionId: string;
  expectedRevision: number;
}): Promise<ExternalImportSelectionMutationResultDto> {
  return invoke<ExternalImportSelectionMutationResultDto>(
    "select_all_external_import_candidates",
    input,
  );
}

export function startExternalImportBatch(input: {
  batchId: string;
  selectionId: string;
  expectedRevision: number;
}): Promise<ExternalImportBatchStartedDto> {
  return invoke<ExternalImportBatchStartedDto>("start_external_import_batch", input);
}

export function cancelExternalImportTask(input: { taskId: string }): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", { taskId: input.taskId });
}

export const cancelExternalImportScan = cancelExternalImportTask;
