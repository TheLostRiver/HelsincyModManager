import assert from "node:assert/strict";
import { test } from "node:test";

import {
  applyInstallManifestStatusSummaries,
  applyInstallManifestUnavailable,
  applyInstallRecoveryUnavailable,
  applyInstallRecoverySummaries,
  resolveLoadedModLibraryItems,
} from "./modLibraryLoadState.ts";

const fallbackItems = [
  {
    id: "mock-mod",
    name: "Mock Mod",
    author: "Mock Author",
    versionLabel: "v1",
    status: "installed",
    categoryLabels: [{ name: "Mock" }],
  },
];

test("successful empty backend mod library result replaces fallback items", () => {
  const result = resolveLoadedModLibraryItems({
    backendItems: [],
    fallbackItems,
  });

  assert.deepEqual(result, []);
});

test("failed backend mod library load keeps fallback items", () => {
  const result = resolveLoadedModLibraryItems({
    backendItems: null,
    fallbackItems,
  });

  assert.equal(result, fallbackItems);
});

test("install manifest summaries override matching mod status without paths", () => {
  const result = applyInstallManifestStatusSummaries(fallbackItems, [
    {
      profileId: "default",
      modId: "mock-mod",
      status: "installed",
      managedFileCount: 2,
      backupCount: 1,
    },
  ]);

  assert.equal(result[0].status, "installed");
  assert.deepEqual(result[0].installSummary, {
    status: "installed",
    managedFileCount: 2,
    backupCount: 1,
  });
  assert.equal("targetPath" in result[0], false);
  assert.equal("manifestPath" in result[0], false);
  assert.equal("backupRoot" in result[0], false);
});

test("install manifest summaries preserve non-manifest mod statuses", () => {
  const result = applyInstallManifestStatusSummaries(
    [
      {
        id: "disabled-mod",
        name: "Disabled Mod",
        status: "disabled",
        sizeLabel: "1 KB",
        categoryLabels: [],
      },
      {
        id: "conflict-mod",
        name: "Conflict Mod",
        status: "conflict",
        sizeLabel: "2 KB",
        categoryLabels: [],
      },
    ],
    [
      {
        profileId: "default",
        modId: "disabled-mod",
        status: "installed",
        managedFileCount: 2,
        backupCount: 0,
      },
      {
        profileId: "default",
        modId: "conflict-mod",
        status: "not_installed",
        managedFileCount: 0,
        backupCount: 0,
      },
    ],
  );

  assert.equal(result[0].status, "disabled");
  assert.deepEqual(result[0].installSummary, {
    status: "installed",
    managedFileCount: 2,
    backupCount: 0,
  });
  assert.equal(result[1].status, "conflict");
  assert.deepEqual(result[1].installSummary, {
    status: "not_installed",
    managedFileCount: 0,
    backupCount: 0,
  });
});

test("install manifest summaries surface cleanup-pending states over display-only statuses", () => {
  const statuses = ["committed_cleanup_pending", "cleanup_pending"];
  const result = applyInstallManifestStatusSummaries(
    statuses.map((status, index) => ({
      id: `cleanup-mod-${index}`,
      name: status,
      status: index === 0 ? "disabled" : "conflict",
      sizeLabel: "1 KB",
      categoryLabels: [],
    })),
    statuses.map((status, index) => ({
      profileId: "default",
      modId: `cleanup-mod-${index}`,
      status,
      managedFileCount: 2,
      backupCount: 1,
    })),
  );

  assert.deepEqual(
    result.map((item) => item.status),
    statuses,
  );
  assert.deepEqual(
    result.map((item) => item.installSummary?.status),
    statuses,
  );
});

test("install recovery summaries map completed status to installed without paths", () => {
  const result = applyInstallRecoverySummaries(fallbackItems, [
    {
      profileId: "default",
      modId: "mock-mod",
      status: "completed",
      managedFileCount: 2,
      backupCount: 1,
      adoptedFileCount: 0,
      issueCount: 0,
      issues: [],
    },
  ]);

  assert.equal(result[0].status, "installed");
  assert.deepEqual(result[0].installSummary, {
    status: "installed",
    managedFileCount: 2,
    backupCount: 1,
    adoptedFileCount: 0,
    recoveryStatus: "completed",
    issueCount: 0,
    issues: [],
  });
  assert.equal("targetPath" in result[0], false);
  assert.equal("manifestPath" in result[0], false);
  assert.equal("backupRef" in result[0], false);
});

test("install recovery summaries surface unsafe states and issue counts", () => {
  const result = applyInstallRecoverySummaries(fallbackItems, [
    {
      profileId: "default",
      modId: "mock-mod",
      status: "repair_required",
      managedFileCount: 3,
      backupCount: 1,
      adoptedFileCount: 1,
      issueCount: 2,
      issues: [{ issue: "target_changed", count: 2 }],
    },
  ]);

  assert.equal(result[0].status, "repair_required");
  assert.deepEqual(result[0].installSummary, {
    status: "repair_required",
    managedFileCount: 3,
    backupCount: 1,
    adoptedFileCount: 1,
    recoveryStatus: "repair_required",
    issueCount: 2,
    issues: [{ issue: "target_changed", count: 2 }],
  });
});

test("install recovery summaries surface rollback-required state as unsafe without paths", () => {
  const result = applyInstallRecoverySummaries(fallbackItems, [
    {
      profileId: "default",
      modId: "mock-mod",
      status: "rollback_required",
      managedFileCount: 2,
      backupCount: 1,
      adoptedFileCount: 0,
      issueCount: 0,
      issues: [],
    },
  ]);

  assert.equal(result[0].status, "rollback_required");
  assert.deepEqual(result[0].installSummary, {
    status: "rollback_required",
    managedFileCount: 2,
    backupCount: 1,
    adoptedFileCount: 0,
    recoveryStatus: "rollback_required",
    issueCount: 0,
    issues: [],
  });
  assert.equal("targetPath" in result[0], false);
  assert.equal("backupRef" in result[0], false);
  assert.equal("manifestPath" in result[0], false);
});

test("install recovery summaries surface cleanup-pending states as unsafe", () => {
  const statuses = ["committed_cleanup_pending", "cleanup_pending"];
  const result = applyInstallRecoverySummaries(
    statuses.map((status, index) => ({
      id: `cleanup-mod-${index}`,
      name: status,
      status: index === 0 ? "disabled" : "conflict",
      sizeLabel: "1 KB",
      categoryLabels: [],
    })),
    statuses.map((status, index) => ({
      profileId: "default",
      modId: `cleanup-mod-${index}`,
      status,
      managedFileCount: 2,
      backupCount: 1,
      issueCount: 0,
      issues: [],
    })),
  );

  assert.deepEqual(
    result.map((item) => item.status),
    statuses,
  );
  assert.deepEqual(
    result.map((item) => item.installSummary?.recoveryStatus),
    statuses,
  );
});

test("unavailable install recovery degrades managed states to unknown without paths", () => {
  const result = applyInstallRecoveryUnavailable([
    {
      id: "installed-mod",
      name: "Installed Mod",
      status: "installed",
      sizeLabel: "1 KB",
      categoryLabels: [],
      installSummary: {
        status: "installed",
        managedFileCount: 2,
        backupCount: 1,
      },
    },
    {
      id: "new-mod",
      name: "New Mod",
      status: "not_installed",
      sizeLabel: "2 KB",
      categoryLabels: [],
      installSummary: {
        status: "not_installed",
        managedFileCount: 0,
        backupCount: 0,
      },
    },
  ]);

  assert.equal(result[0].status, "unknown");
  assert.deepEqual(result[0].installSummary, {
    status: "unknown",
    managedFileCount: 2,
    backupCount: 1,
    recoveryStatus: "unknown",
    issueCount: 0,
    issues: [],
  });
  assert.equal(result[1].status, "not_installed");
  assert.deepEqual(result[1].installSummary, {
    status: "not_installed",
    managedFileCount: 0,
    backupCount: 0,
  });
  assert.equal("targetPath" in result[0], false);
  assert.equal("backupRef" in result[0], false);
});

test("unavailable manifest status fails closed even for not-installed and legacy display states", () => {
  const result = applyInstallManifestUnavailable([
    {
      id: "not-installed-mod",
      name: "Not installed Mod",
      status: "not_installed",
      sizeLabel: "1 KB",
      categoryLabels: [],
      installSummary: {
        status: "not_installed",
        managedFileCount: 0,
        backupCount: 0,
      },
    },
    {
      id: "legacy-disabled-mod",
      name: "Legacy Disabled Mod",
      status: "disabled",
      sizeLabel: "2 KB",
      categoryLabels: [],
    },
  ]);

  assert.deepEqual(result.map((item) => item.status), ["unknown", "unknown"]);
  assert.deepEqual(result.map((item) => item.installSummary?.status), ["unknown", "unknown"]);
  assert.deepEqual(result[1].installSummary, {
    status: "unknown",
    managedFileCount: 0,
    backupCount: 0,
    recoveryStatus: "unknown",
    issueCount: 0,
    issues: [],
  });
});

test("unavailable manifest status preserves existing durable counters while failing closed", () => {
  const [result] = applyInstallManifestUnavailable([
    {
      id: "installed-mod",
      name: "Installed Mod",
      status: "installed",
      sizeLabel: "3 KB",
      categoryLabels: [],
      installSummary: {
        status: "installed",
        managedFileCount: 4,
        backupCount: 2,
        recoveryStatus: "completed",
        issueCount: 0,
        issues: [],
      },
    },
  ]);

  assert.equal(result.status, "unknown");
  assert.deepEqual(result.installSummary, {
    status: "unknown",
    managedFileCount: 4,
    backupCount: 2,
    recoveryStatus: "unknown",
    issueCount: 0,
    issues: [],
  });
});

// #286 adopt 收尾：接管计数只从后端摘要透传，缺席就缺席，不补 0（0 与「不知道」是两回事）。
test("manifest summaries carry the adopted count through and leave it absent when the backend omits it", () => {
  const [carried, omitted] = applyInstallManifestStatusSummaries(
    [
      { id: "adopted-mod", name: "Adopted", status: "installed", sizeLabel: "1 KB", categoryLabels: [] },
      { id: "plain-mod", name: "Plain", status: "installed", sizeLabel: "1 KB", categoryLabels: [] },
    ],
    [
      {
        profileId: "default",
        modId: "adopted-mod",
        status: "installed",
        managedFileCount: 3,
        backupCount: 1,
        installedRevisionId: null,
        adoptedFileCount: 2,
      },
      {
        profileId: "default",
        modId: "plain-mod",
        status: "installed",
        managedFileCount: 1,
        backupCount: 1,
        installedRevisionId: null,
      },
    ],
  );

  assert.equal(carried.installSummary.adoptedFileCount, 2);
  assert.equal("adoptedFileCount" in omitted.installSummary, false);
});

test("unavailable manifest status keeps a previously known adopted count and never invents one", () => {
  const [known, unknown] = applyInstallManifestUnavailable([
    {
      id: "adopted-mod",
      name: "Adopted",
      status: "installed",
      sizeLabel: "1 KB",
      categoryLabels: [],
      installSummary: { status: "installed", managedFileCount: 3, backupCount: 1, adoptedFileCount: 2 },
    },
    {
      id: "plain-mod",
      name: "Plain",
      status: "installed",
      sizeLabel: "1 KB",
      categoryLabels: [],
      installSummary: { status: "installed", managedFileCount: 1, backupCount: 1 },
    },
  ]);

  assert.equal(known.installSummary.adoptedFileCount, 2);
  assert.equal("adoptedFileCount" in unknown.installSummary, false);
});
