import type { TaskStartedDto } from "../modImportTypes";

export const EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE = 50;
export const EXTERNAL_IMPORT_DISPLAY_TEXT_MAX_LENGTH = 160;

const EXTERNAL_IMPORT_OPAQUE_ID_PATTERN = /^[A-Za-z0-9_-]{1,160}$/;

export function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isExternalImportOpaqueId(value: unknown): value is string {
  return typeof value === "string" && EXTERNAL_IMPORT_OPAQUE_ID_PATTERN.test(value);
}

export function isExternalImportDisplayText(value: unknown): value is string {
  return (
    typeof value === "string" &&
    Array.from(value).length <= EXTERNAL_IMPORT_DISPLAY_TEXT_MAX_LENGTH &&
    !Array.from(value).some((character) => {
      const code = character.charCodeAt(0);
      return code < 32 || code === 127;
    })
  );
}

export function isOptionalDisplayText(value: unknown): value is string | null {
  return value === null || isExternalImportDisplayText(value);
}

export function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

export type ExternalImportSourceDto = {
  sourceId: string;
  adapterId: string;
  displayLabel: string;
  expiresAtUnixMillis: number;
};

export function isExternalImportSourceDto(value: unknown): value is ExternalImportSourceDto {
  if (!isPlainRecord(value)) {
    return false;
  }

  return (
    isExternalImportOpaqueId(value.sourceId) &&
    isExternalImportOpaqueId(value.adapterId) &&
    isExternalImportDisplayText(value.displayLabel) &&
    value.displayLabel.trim().length > 0 &&
    !/[\\/:]/.test(value.displayLabel) &&
    isSafeNonNegativeInteger(value.expiresAtUnixMillis)
  );
}

export type ExternalImportScanStartedDto = {
  task: TaskStartedDto;
  batchId: string;
};

export type ExternalImportScanStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export type ExternalImportBatchImportStatus =
  | "pending"
  | "running"
  | "completed"
  | "completed_with_errors"
  | "failed"
  | "cancelled";

export type ExternalImportCandidateStatus =
  | "ready"
  | "already_imported"
  | "duplicate_in_batch"
  | "name_collision"
  | "structure_invalid"
  | "metadata_invalid"
  | "unsupported_entry"
  | "resource_limit_exceeded"
  | "source_unreadable";

export type ExternalImportConflictKind = "none" | "content_duplicate" | "name_collision";

export type ExternalImportMetadataHintDto = {
  displayName: string | null;
  author: string | null;
  version: string | null;
  sourceModType: string | null;
};

export type ExternalImportPreviewBatchDto = {
  batchId: string;
  adapterId: string;
  scanStatus: ExternalImportScanStatus;
  importStatus: ExternalImportBatchImportStatus;
};

export type ExternalImportPreviewCandidateDto = {
  candidateId: string;
  metadata: ExternalImportMetadataHintDto;
  fileCount: number;
  totalBytes: number;
  previewStatus: ExternalImportCandidateStatus;
  conflictKind: ExternalImportConflictKind;
  reasonCode: string | null;
};

export type ExternalImportPreviewPageDto = {
  batch: ExternalImportPreviewBatchDto;
  candidates: ExternalImportPreviewCandidateDto[];
  totalCount: number;
  nextCursor: string | null;
};
