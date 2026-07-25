import assert from "node:assert/strict";
import { test } from "node:test";

import {
  appendExternalImportPreviewCandidates,
  isExternalImportPreviewPageForBatch,
  toExternalImportPreviewCandidateViewModel,
} from "./externalImportPreviewModel.ts";

function candidate(overrides = {}) {
  return {
    candidateId: "candidate-a",
    metadata: {
      displayName: "人工候选",
      author: "测试作者",
      version: "1.0.0",
      sourceModType: "外观",
    },
    fileCount: 12,
    totalBytes: 2048,
    previewStatus: "ready",
    conflictKind: "none",
    reasonCode: null,
    ...overrides,
  };
}

function previewPage(overrides = {}) {
  return {
    batch: {
      batchId: "batch-a",
      adapterId: "hunting_box_directory_v1",
      scanStatus: "completed",
      importStatus: "pending",
    },
    candidates: [candidate()],
    totalCount: 1,
    nextCursor: null,
    ...overrides,
  };
}

test("preview page accepts only the completed pending batch shape and an opaque cursor", () => {
  assert.equal(isExternalImportPreviewPageForBatch(previewPage(), "batch-a"), true);
  assert.equal(
    isExternalImportPreviewPageForBatch(previewPage({ nextCursor: "next/page" }), "batch-a"),
    false,
  );
  assert.equal(
    isExternalImportPreviewPageForBatch(
      previewPage({ batch: { ...previewPage().batch, importStatus: "running" } }),
      "batch-a",
    ),
    false,
  );
});

test("candidate view model is text-only and unknown statuses fail closed", () => {
  const known = toExternalImportPreviewCandidateViewModel(candidate());
  assert.equal(known.title, "人工候选");
  assert.equal(known.fileCount, "12 个文件");
  assert.equal(known.totalBytes, "2 KB");
  assert.equal(known.statusLabel, "可导入");

  const unknown = toExternalImportPreviewCandidateViewModel(candidate({ previewStatus: "future_status" }));
  assert.equal(unknown.statusLabel, "需要重新扫描");
  assert.equal(unknown.statusTone, "danger");
});

test("preview validation rejects unsafe metadata, imprecise counts, and oversized pages", () => {
  const controlCharacter = String.fromCharCode(0);

  assert.equal(
    isExternalImportPreviewPageForBatch(
      previewPage({
        candidates: [candidate({ metadata: { ...candidate().metadata, displayName: `unsafe${controlCharacter}name` } })],
      }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportPreviewPageForBatch(previewPage({ candidates: [candidate({ fileCount: 1.5 })] }), "batch-a"),
    false,
  );

  const oversizedCandidates = Array.from({ length: 51 }, (_, index) =>
    candidate({ candidateId: `candidate-${index}` }),
  );
  assert.equal(
    isExternalImportPreviewPageForBatch(
      previewPage({ candidates: oversizedCandidates, totalCount: oversizedCandidates.length }),
      "batch-a",
    ),
    false,
  );
});

test("preview pagination deduplicates candidate ids before presentation", () => {
  const existing = [toExternalImportPreviewCandidateViewModel(candidate())];
  const merged = appendExternalImportPreviewCandidates(existing, [
    candidate(),
    candidate({ candidateId: "candidate-b", metadata: { ...candidate().metadata, displayName: "第二项" } }),
  ]);

  assert.equal(merged.length, 2);
  assert.deepEqual(merged.map((item) => item.candidateId), ["candidate-a", "candidate-b"]);
});
