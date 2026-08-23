import assert from "node:assert/strict";
import { test } from "node:test";

import { externalImportCopy } from "./externalImportCopy.ts";
import {
  applyExternalImportSelectionMutationResult,
  canSelectExternalImportCandidateWithDecision,
  getExternalImportSelectionErrorMessage,
  getRequiredExternalImportConflictResolution,
  isExternalImportBatchStartedDto,
  isExternalImportCandidateSelectionFactValid,
  isExternalImportSelectionCategory,
  isExternalImportSelectionDto,
  isExternalImportSelectionExpired,
  isExternalImportSelectionMutationResultDto,
} from "./externalImportSelectionModel.ts";

const zhSelection = externalImportCopy.zh_cn.selection;

function selection(overrides = {}) {
  return {
    selectionId: "selection-a",
    revision: 2,
    status: "editing",
    selectedCount: 1,
    selectedResourceUsage: {
      fileCount: 3,
      sourceBytes: 1024,
      materializationBytes: 2048,
    },
    expiresAtUnixMillis: 10_000,
    ...overrides,
  };
}

test("selection guards accept only opaque identities, stable status, and safe aggregate counts", () => {
  assert.equal(isExternalImportSelectionDto(selection()), true);
  assert.equal(isExternalImportSelectionDto(selection({ selectionId: "selection/path" })), false);
  assert.equal(isExternalImportSelectionDto(selection({ revision: 1.5 })), false);
  assert.equal(isExternalImportSelectionDto(selection({ status: "future" })), false);
  assert.equal(
    isExternalImportSelectionDto(
      selection({ selectedResourceUsage: { ...selection().selectedResourceUsage, sourceBytes: -1 } }),
    ),
    false,
  );

  assert.equal(
    isExternalImportSelectionMutationResultDto({
      revision: 3,
      selectedCount: 2,
      selectedResourceUsage: selection().selectedResourceUsage,
    }),
    true,
  );
  assert.equal(
    isExternalImportSelectionMutationResultDto({
      revision: 3,
      selectedCount: Number.MAX_SAFE_INTEGER + 1,
      selectedResourceUsage: selection().selectedResourceUsage,
    }),
    false,
  );
});

test("selection mutation result updates only server-owned revision and aggregate facts", () => {
  const current = selection();
  const next = applyExternalImportSelectionMutationResult(current, {
    revision: 3,
    selectedCount: 4,
    selectedResourceUsage: {
      fileCount: 9,
      sourceBytes: 4096,
      materializationBytes: 8192,
    },
  });

  assert.deepEqual(next, {
    ...current,
    revision: 3,
    selectedCount: 4,
    selectedResourceUsage: {
      fileCount: 9,
      sourceBytes: 4096,
      materializationBytes: 8192,
    },
  });
});

test("selection expiry uses the backend deadline and only expires editing snapshots", () => {
  assert.equal(isExternalImportSelectionExpired(selection(), 9_999), false);
  assert.equal(isExternalImportSelectionExpired(selection(), 10_000), true);
  assert.equal(
    isExternalImportSelectionExpired(selection({ status: "sealed" }), 10_001),
    false,
  );
  assert.equal(
    isExternalImportSelectionExpired(selection({ status: "expired" }), 0),
    true,
  );
});

test("candidate decisions remain explicit and blocked statuses fail closed", () => {
  assert.equal(getRequiredExternalImportConflictResolution("ready"), null);
  assert.equal(getRequiredExternalImportConflictResolution("name_collision"), "keep_both");
  assert.equal(
    getRequiredExternalImportConflictResolution("metadata_invalid"),
    "ignore_invalid_metadata",
  );
  assert.equal(getRequiredExternalImportConflictResolution("future_status"), "unsupported");

  assert.equal(canSelectExternalImportCandidateWithDecision("ready", null), true);
  assert.equal(canSelectExternalImportCandidateWithDecision("name_collision", null), false);
  assert.equal(
    canSelectExternalImportCandidateWithDecision("name_collision", "keep_both"),
    true,
  );
  assert.equal(canSelectExternalImportCandidateWithDecision("already_imported", null), false);
});

test("candidate selection facts mirror the authoritative status and decision matrix", () => {
  const categoryOnly = { conflictResolution: null, categoryId: "category-a" };
  const keepBoth = { conflictResolution: "keep_both", categoryId: null };
  const ignoreInvalidMetadata = {
    conflictResolution: "ignore_invalid_metadata",
    categoryId: null,
  };

  assert.equal(
    isExternalImportCandidateSelectionFactValid("ready", false, null),
    true,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid("ready", false, categoryOnly),
    false,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid("ready", true, null),
    true,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid("ready", true, categoryOnly),
    true,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid("ready", true, keepBoth),
    true,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid(
      "ready",
      true,
      ignoreInvalidMetadata,
    ),
    false,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid(
      "name_collision",
      true,
      keepBoth,
    ),
    true,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid("name_collision", true, null),
    false,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid(
      "metadata_invalid",
      true,
      ignoreInvalidMetadata,
    ),
    true,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid(
      "metadata_invalid",
      true,
      keepBoth,
    ),
    false,
  );
  assert.equal(
    isExternalImportCandidateSelectionFactValid(
      "already_imported",
      true,
      null,
    ),
    false,
  );
});

test("selection category guard accepts nullable backend colors and rejects invalid colors", () => {
  const category = {
    id: "category-a",
    name: "人工分类",
    color: null,
    sortOrder: 0,
    modCount: 0,
  };

  assert.equal(isExternalImportSelectionCategory(category), true);
  assert.equal(
    isExternalImportSelectionCategory({ ...category, color: "#0EA5E9" }),
    true,
  );
  assert.equal(
    isExternalImportSelectionCategory({ ...category, color: undefined }),
    true,
  );
  assert.equal(
    isExternalImportSelectionCategory({ ...category, color: 42 }),
    false,
  );
  assert.equal(
    isExternalImportSelectionCategory({
      ...category,
      color: `unsafe${String.fromCharCode(0)}color`,
    }),
    false,
  );
});

test("batch launch guard requires the same batch and a queued mod import task", () => {
  const launch = {
    task: { taskId: "mod-import-1", kind: "mod_import", status: "queued" },
    batchId: "batch-a",
  };

  assert.equal(isExternalImportBatchStartedDto(launch, "batch-a"), true);
  assert.equal(isExternalImportBatchStartedDto(launch, "batch-b"), false);
  assert.equal(
    isExternalImportBatchStartedDto(
      { ...launch, task: { ...launch.task, kind: "install" } },
      "batch-a",
    ),
    false,
  );
});

test("selection error mapping branches only on stable codes", () => {
  assert.equal(
    getExternalImportSelectionErrorMessage("selection_revision_conflict", zhSelection),
    "选择已发生变化，已重新载入",
  );
  assert.equal(
    getExternalImportSelectionErrorMessage("C:\\private\\raw-error", zhSelection),
    "无法更新候选选择，请重新载入后重试",
  );
});
