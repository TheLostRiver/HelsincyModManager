import assert from "node:assert/strict";
import { test } from "node:test";

import { resolveLoadedModLibraryItems } from "./modLibraryLoadState.ts";

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
