import assert from "node:assert/strict";
import { test } from "node:test";

import {
  applyInstallManifestStatusSummaries,
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
    categoryLabels: ["Mock"],
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

test("install recovery summaries map completed status to installed without paths", () => {
  const result = applyInstallRecoverySummaries(fallbackItems, [
    {
      profileId: "default",
      modId: "mock-mod",
      status: "completed",
      managedFileCount: 2,
      backupCount: 1,
      issueCount: 0,
      issues: [],
    },
  ]);

  assert.equal(result[0].status, "installed");
  assert.deepEqual(result[0].installSummary, {
    status: "installed",
    managedFileCount: 2,
    backupCount: 1,
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
      issueCount: 2,
      issues: [{ issue: "target_changed", count: 2 }],
    },
  ]);

  assert.equal(result[0].status, "repair_required");
  assert.deepEqual(result[0].installSummary, {
    status: "repair_required",
    managedFileCount: 3,
    backupCount: 1,
    recoveryStatus: "repair_required",
    issueCount: 2,
    issues: [{ issue: "target_changed", count: 2 }],
  });
});
