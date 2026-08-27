import {
  EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE,
  isExternalImportDisplayText,
  isExternalImportOpaqueId,
  isOptionalDisplayText,
  isPlainRecord,
  isSafeNonNegativeInteger,
  type ExternalImportPreviewCandidateDto,
  type ExternalImportPreviewPageDto,
} from "./externalImportTypes.ts";
import {
  isExternalImportCandidateSelectionFactValid,
  isExternalImportSelectionDto,
} from "./externalImportSelectionModel.ts";

export type ExternalImportPreviewCandidateViewModel = {
  candidateId: string;
  title: string;
  metadata: string[];
  fileCount: string;
  totalBytes: string;
  statusLabel: string;
  statusTone: "ready" | "warning" | "danger";
  conflictLabel: string | null;
  previewStatus: ExternalImportPreviewCandidateDto["previewStatus"];
  selected: boolean;
  selectionDecision: ExternalImportPreviewCandidateDto["selectionDecision"];
};

import type { ExternalImportCopy } from "./externalImportCopy";

const previewStatusTones: Readonly<
  Record<string, ExternalImportPreviewCandidateViewModel["statusTone"]>
> = {
  ready: "ready",
  already_imported: "warning",
  duplicate_in_batch: "warning",
  name_collision: "warning",
  structure_invalid: "danger",
  metadata_invalid: "warning",
  unsupported_entry: "danger",
  resource_limit_exceeded: "danger",
  source_unreadable: "danger",
  payload_missing: "warning",
};



function isCandidateShape(value: unknown): value is ExternalImportPreviewCandidateDto {
  if (!isPlainRecord(value) || !isExternalImportOpaqueId(value.candidateId) || !isPlainRecord(value.metadata)) {
    return false;
  }

  const metadata = value.metadata;
  return (
    isOptionalDisplayText(metadata.displayName) &&
    isOptionalDisplayText(metadata.author) &&
    isOptionalDisplayText(metadata.version) &&
    isOptionalDisplayText(metadata.sourceModType) &&
    typeof value.previewStatus === "string" &&
    typeof value.conflictKind === "string" &&
    isOptionalDisplayText(value.reasonCode) &&
    isSafeNonNegativeInteger(value.fileCount) &&
    isSafeNonNegativeInteger(value.totalBytes) &&
    isExternalImportCandidateSelectionFactValid(
      value.previewStatus,
      value.selected,
      value.selectionDecision,
    )
  );
}

export function isExternalImportPreviewPageForBatch(
  value: unknown,
  batchId: string,
  expectedSelectionId: string | null = null,
): value is ExternalImportPreviewPageDto {
  if (!isPlainRecord(value) || !isPlainRecord(value.batch) || !Array.isArray(value.candidates)) {
    return false;
  }

  const batch = value.batch;
  const totalCount = value.totalCount;
  if (!isSafeNonNegativeInteger(totalCount)) {
    return false;
  }
  const selectionMatches =
    expectedSelectionId === null
      ? value.selection === null
      : isExternalImportSelectionDto(value.selection) &&
        value.selection.selectionId === expectedSelectionId;
  const selectionCountIsValid =
    value.selection === null ||
    (isExternalImportSelectionDto(value.selection) &&
      value.selection.selectedCount <= totalCount);
  const candidatesAreValid = value.candidates.every(
    (candidate) =>
      isCandidateShape(candidate) &&
      (value.selection !== null || candidate.selected === false),
  );
  return (
    batch.batchId === batchId &&
    isExternalImportOpaqueId(batch.batchId) &&
    isExternalImportOpaqueId(batch.adapterId) &&
    batch.scanStatus === "completed" &&
    batch.importStatus === "pending" &&
    selectionMatches &&
    selectionCountIsValid &&
    totalCount >= value.candidates.length &&
    value.candidates.length <= EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE &&
    (value.nextCursor === null || (typeof value.nextCursor === "string" && /^\d+$/.test(value.nextCursor))) &&
    candidatesAreValid
  );
}

function safeDisplayText(value: string | null, fallback: string | null = null) {
  if (!isExternalImportDisplayText(value)) {
    return fallback;
  }

  const normalized = value.trim();
  return normalized || fallback;
}

function formatInteger(value: number) {
  return new Intl.NumberFormat("zh-CN").format(Math.trunc(value));
}

function formatByteCount(value: number) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let unitIndex = 0;
  let scaled = Math.max(0, value);

  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  const digits = unitIndex === 0 || scaled >= 100 ? 0 : scaled >= 10 ? 1 : 2;
  return `${new Intl.NumberFormat("zh-CN", { maximumFractionDigits: digits }).format(scaled)} ${units[unitIndex]}`;
}

export function toExternalImportPreviewCandidateViewModel(
  candidate: ExternalImportPreviewCandidateDto,
  preview: ExternalImportCopy["preview"],
): ExternalImportPreviewCandidateViewModel {
  const statusLabel = preview.status[candidate.previewStatus] ?? preview.rescan;
  const statusTone = previewStatusTones[candidate.previewStatus] ?? ("danger" as const);
  const metadata = [
    safeDisplayText(candidate.metadata.author),
    safeDisplayText(candidate.metadata.version),
    safeDisplayText(candidate.metadata.sourceModType),
  ].filter((value): value is string => value !== null);

  return {
    candidateId: candidate.candidateId,
    title: safeDisplayText(candidate.metadata.displayName, preview.unnamed) ?? preview.unnamed,
    metadata,
    fileCount: preview.fileCount(formatInteger(candidate.fileCount)),
    totalBytes: formatByteCount(candidate.totalBytes),
    statusLabel,
    statusTone,
    conflictLabel:
      candidate.conflictKind === "none"
        ? null
        : preview.conflicts[candidate.conflictKind] ?? preview.needsReview,
    previewStatus: candidate.previewStatus,
    selected: candidate.selected,
    selectionDecision: candidate.selectionDecision,
  };
}

export function appendExternalImportPreviewCandidates(
  existing: ExternalImportPreviewCandidateViewModel[],
  incoming: ExternalImportPreviewCandidateDto[],
  preview: ExternalImportCopy["preview"],
) {
  const seenCandidateIds = new Set(existing.map((candidate) => candidate.candidateId));
  const next = [...existing];

  for (const candidate of incoming) {
    if (seenCandidateIds.has(candidate.candidateId)) {
      continue;
    }
    seenCandidateIds.add(candidate.candidateId);
    next.push(toExternalImportPreviewCandidateViewModel(candidate, preview));
  }

  return next;
}
