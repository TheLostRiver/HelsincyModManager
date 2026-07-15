import assert from "node:assert/strict";
import { test } from "node:test";

import { deriveInstallRecoveryHealth } from "../install-recovery/installRecoveryHealth.ts";

const baseSummary = {
  profileId: "default",
  managedFileCount: 0,
  backupCount: 0,
  issueCount: 0,
  issues: [],
};

test("derives healthy app recovery state from completed and not installed summaries", () => {
  const summary = deriveInstallRecoveryHealth([
    {
      ...baseSummary,
      modId: "mod-a",
      status: "completed",
      managedFileCount: 2,
      backupCount: 1,
    },
    {
      ...baseSummary,
      modId: "mod-b",
      status: "not_installed",
    },
  ]);

  assert.equal(summary.status, "healthy");
  assert.equal(summary.scannedModCount, 2);
  assert.equal(summary.completedModCount, 1);
  assert.equal(summary.attentionModCount, 0);
  assert.equal(summary.unknownModCount, 0);
  assert.equal(summary.managedFileCount, 2);
  assert.equal(summary.backupCount, 1);
  assert.equal(summary.issueCount, 0);
  assert.deepEqual(summary.issues, []);
});

test("derives attention state and aggregates recovery issues without paths", () => {
  const summary = deriveInstallRecoveryHealth([
    {
      ...baseSummary,
      modId: "mod-a",
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
      modId: "mod-b",
      status: "unknown",
      managedFileCount: 1,
      issueCount: 1,
      issues: [{ issue: "target_read_failed", count: 1 }],
    },
  ]);

  assert.equal(summary.status, "attention");
  assert.equal(summary.scannedModCount, 2);
  assert.equal(summary.completedModCount, 0);
  assert.equal(summary.attentionModCount, 1);
  assert.equal(summary.unknownModCount, 1);
  assert.equal(summary.managedFileCount, 4);
  assert.equal(summary.backupCount, 1);
  assert.equal(summary.issueCount, 4);
  assert.deepEqual(summary.issues, [
    { issue: "target_changed", count: 2 },
    { issue: "target_read_failed", count: 1 },
    { issue: "backup_missing", count: 1 },
  ]);
  assert.equal("targetPath" in summary, false);
  assert.equal("backupRef" in summary, false);
  assert.equal("manifestPath" in summary, false);
});

test("derives attention state from rollback-required records without issue noise", () => {
  const summary = deriveInstallRecoveryHealth([
    {
      ...baseSummary,
      modId: "rollback-mod",
      status: "rollback_required",
      managedFileCount: 2,
      backupCount: 1,
    },
  ]);

  assert.equal(summary.status, "attention");
  assert.equal(summary.scannedModCount, 1);
  assert.equal(summary.completedModCount, 0);
  assert.equal(summary.attentionModCount, 1);
  assert.equal(summary.unknownModCount, 0);
  assert.equal(summary.managedFileCount, 2);
  assert.equal(summary.backupCount, 1);
  assert.equal(summary.issueCount, 0);
  assert.deepEqual(summary.issues, []);
  assert.equal("targetPath" in summary, false);
  assert.equal("backupRef" in summary, false);
});

test("derives attention state from cleanup-pending records", () => {
  const summary = deriveInstallRecoveryHealth([
    {
      ...baseSummary,
      modId: "committed-mod",
      status: "committed_cleanup_pending",
      managedFileCount: 2,
      backupCount: 1,
    },
    {
      ...baseSummary,
      modId: "cleanup-mod",
      status: "cleanup_pending",
      managedFileCount: 1,
    },
  ]);

  assert.equal(summary.status, "attention");
  assert.equal(summary.scannedModCount, 2);
  assert.equal(summary.completedModCount, 0);
  assert.equal(summary.attentionModCount, 2);
  assert.equal(summary.unknownModCount, 0);
  assert.equal(summary.managedFileCount, 3);
  assert.equal(summary.backupCount, 1);
  assert.equal(summary.issueCount, 0);
});

test("derives empty state when profile scan has no managed mods", () => {
  const summary = deriveInstallRecoveryHealth([]);

  assert.deepEqual(summary, {
    status: "empty",
    scannedModCount: 0,
    completedModCount: 0,
    attentionModCount: 0,
    unknownModCount: 0,
    managedFileCount: 0,
    backupCount: 0,
    issueCount: 0,
    issues: [],
  });
});
