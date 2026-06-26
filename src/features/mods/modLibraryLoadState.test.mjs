import assert from "node:assert/strict";
import { test } from "node:test";

import { applyInstallManifestStatusSummaries, resolveLoadedModLibraryItems } from "./modLibraryLoadState.ts";

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
