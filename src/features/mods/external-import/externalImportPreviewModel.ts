import {
  EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE,
  isExternalImportDisplayText,
  isExternalImportOpaqueId,
  isPlainRecord,
  type ExternalImportPreviewCandidateDto,
  type ExternalImportPreviewPageDto,
} from "./externalImportTypes.ts";

export type ExternalImportPreviewCandidateViewModel = {
  candidateId: string;
  title: string;
  metadata: string[];
  fileCount: string;
  totalBytes: string;
  statusLabel: string;
  statusTone: "ready" | "warning" | "danger";
  conflictLabel: string | null;
};

const previewStatusPresentation: Readonly<
  Record<string, Pick<ExternalImportPreviewCandidateViewModel, "statusLabel" | "statusTone">>
> = {
  ready: { statusLabel: "可导入", statusTone: "ready" },
  already_imported: { statusLabel: "已存在", statusTone: "warning" },
  duplicate_in_batch: { statusLabel: "批次重复", statusTone: "warning" },
  name_collision: { statusLabel: "名称冲突", statusTone: "warning" },
  structure_invalid: { statusLabel: "结构不可用", statusTone: "danger" },
  metadata_invalid: { statusLabel: "元数据不可用", statusTone: "warning" },
  unsupported_entry: { statusLabel: "不支持的条目", statusTone: "danger" },
  resource_limit_exceeded: { statusLabel: "超出资源限制", statusTone: "danger" },
  source_unreadable: { statusLabel: "来源不可读取", statusTone: "danger" },
};

const conflictLabels: Readonly<Record<string, string | null>> = {
  none: null,
  content_duplicate: "内容重复",
  name_collision: "同名冲突",
};

function isOptionalDisplayText(value: unknown) {
  return value === null || isExternalImportDisplayText(value);
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isCandidateShape(value: unknown) {
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
    isSafeNonNegativeInteger(value.totalBytes)
  );
}

export function isExternalImportPreviewPageForBatch(
  value: unknown,
  batchId: string,
): value is ExternalImportPreviewPageDto {
  if (!isPlainRecord(value) || !isPlainRecord(value.batch) || !Array.isArray(value.candidates)) {
    return false;
  }

  const batch = value.batch;
  const totalCount = value.totalCount;
  return (
    batch.batchId === batchId &&
    isExternalImportOpaqueId(batch.batchId) &&
    isExternalImportOpaqueId(batch.adapterId) &&
    batch.scanStatus === "completed" &&
    batch.importStatus === "pending" &&
    isSafeNonNegativeInteger(totalCount) &&
    totalCount >= value.candidates.length &&
    value.candidates.length <= EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE &&
    (value.nextCursor === null || (typeof value.nextCursor === "string" && /^\d+$/.test(value.nextCursor))) &&
    value.candidates.every(isCandidateShape)
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
): ExternalImportPreviewCandidateViewModel {
  const presentation = previewStatusPresentation[candidate.previewStatus] ?? {
    statusLabel: "需要重新扫描",
    statusTone: "danger" as const,
  };
  const metadata = [
    safeDisplayText(candidate.metadata.author),
    safeDisplayText(candidate.metadata.version),
    safeDisplayText(candidate.metadata.sourceModType),
  ].filter((value): value is string => value !== null);

  return {
    candidateId: candidate.candidateId,
    title: safeDisplayText(candidate.metadata.displayName, "未命名候选") ?? "未命名候选",
    metadata,
    fileCount: `${formatInteger(candidate.fileCount)} 个文件`,
    totalBytes: formatByteCount(candidate.totalBytes),
    statusLabel: presentation.statusLabel,
    statusTone: presentation.statusTone,
    conflictLabel: conflictLabels[candidate.conflictKind] ?? "需要复核",
  };
}

export function appendExternalImportPreviewCandidates(
  existing: ExternalImportPreviewCandidateViewModel[],
  incoming: ExternalImportPreviewCandidateDto[],
) {
  const seenCandidateIds = new Set(existing.map((candidate) => candidate.candidateId));
  const next = [...existing];

  for (const candidate of incoming) {
    if (seenCandidateIds.has(candidate.candidateId)) {
      continue;
    }
    seenCandidateIds.add(candidate.candidateId);
    next.push(toExternalImportPreviewCandidateViewModel(candidate));
  }

  return next;
}
