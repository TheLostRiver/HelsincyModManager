import assert from "node:assert/strict";
import { test } from "node:test";

import { deriveRecoveryCenterViewModel } from "./recoveryCenterViewModel.ts";

const baseSummary = {
  profileId: "default",
  managedFileCount: 0,
  backupCount: 0,
  issueCount: 0,
  issues: [],
};

test("derives profile recovery center overview without path fields", () => {
  const viewModel = deriveRecoveryCenterViewModel([
    {
      ...baseSummary,
      modId: "healthy-mod",
      status: "completed",
      managedFileCount: 2,
      backupCount: 1,
    },
    {
      ...baseSummary,
      modId: "changed-mod",
      status: "repair_required",
      managedFileCount: 3,
      backupCount: 1,
      issueCount: 3,
      issues: [
        { issue: "target_changed", count: 2 },
        { issue: "backup_missing", count: 1 },
      ],
    },
    {
      ...baseSummary,
      modId: "unknown-mod",
      status: "unknown",
      managedFileCount: 1,
      issueCount: 1,
      issues: [{ issue: "target_read_failed", count: 1 }],
    },
  ]);

  assert.equal(viewModel.overview.status, "attention");
  assert.equal(viewModel.overview.scannedModCount, 3);
  assert.equal(viewModel.overview.completedModCount, 1);
  assert.equal(viewModel.overview.attentionModCount, 1);
  assert.equal(viewModel.overview.unknownModCount, 1);
  assert.equal(viewModel.overview.managedFileCount, 6);
  assert.equal(viewModel.overview.backupCount, 2);
  assert.equal(viewModel.overview.issueCount, 4);
  assert.deepEqual(viewModel.overview.issues, [
    { issue: "target_changed", count: 2, label: "目标变更" },
    { issue: "target_read_failed", count: 1, label: "读取未知" },
    { issue: "backup_missing", count: 1, label: "备份缺失" },
  ]);
  assert.deepEqual(
    viewModel.mods.map((mod) => [mod.modId, mod.status, mod.statusLabel]),
    [
      ["changed-mod", "repair_required", "需要修复"],
      ["unknown-mod", "unknown", "状态未知"],
      ["healthy-mod", "completed", "正常"],
    ],
  );
  assert.equal("targetPath" in viewModel.overview, false);
  assert.equal("gameRoot" in viewModel.overview, false);
  assert.equal("backupRef" in viewModel.mods[0], false);
  assert.equal("manifestPath" in viewModel.mods[0], false);
});

test("derives empty recovery center state for a profile without managed installs", () => {
  const viewModel = deriveRecoveryCenterViewModel([]);

  assert.equal(viewModel.overview.status, "empty");
  assert.equal(viewModel.mods.length, 0);
  assert.equal(viewModel.overview.scannedModCount, 0);
  assert.equal(viewModel.overview.issueCount, 0);
});
