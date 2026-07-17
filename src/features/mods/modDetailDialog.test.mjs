import assert from "node:assert/strict";
import { test } from "node:test";

import { getTrappedFocusIndex } from "../../shared/feedback/focusTrap.ts";
import {
  createDetailDialogState,
  loadModLibraryItemsForMode,
  preserveItemsOnRefreshFailure,
} from "./modLibraryRefresh.ts";
import {
  parseNexusModId,
  saveModDetailChanges,
  selectedIdsFromCategories,
} from "./modDetailDialogWorkflow.ts";

const fallbackItems = [
  {
    id: "mod-a",
    name: "Alpha",
    status: "installed",
    sizeLabel: "12 MB",
    categoryLabels: [{ name: "外观" }],
  },
];

test("saveModDetailChanges reports category failure after metadata has been saved", async () => {
  const calls = [];

  const result = await saveModDetailChanges({
    modId: "mod-a",
    metadata: {
      displayName: "Alpha Edit",
      author: "Helsincy",
      version: "1.2.3",
      description: "note",
      nexusModId: 42,
    },
    categoryIds: ["cat-a"],
    categoriesReady: true,
    updateModMetadata: async () => {
      calls.push("metadata");
    },
    setModCategories: async () => {
      calls.push("categories");
      throw new Error("category write failed");
    },
    onSaved: async () => {
      calls.push("refresh");
    },
  });

  assert.deepEqual(calls, ["metadata", "categories", "refresh"]);
  assert.deepEqual(result, { status: "partial-category-failure" });
});

test("saveModDetailChanges skips category writes when category data is unavailable", async () => {
  const calls = [];

  const result = await saveModDetailChanges({
    modId: "mod-a",
    metadata: { displayName: "Alpha Edit" },
    categoryIds: ["cat-a"],
    categoriesReady: false,
    updateModMetadata: async () => {
      calls.push("metadata");
    },
    setModCategories: async () => {
      calls.push("categories");
    },
    onSaved: async () => {
      calls.push("refresh");
    },
  });

  assert.deepEqual(calls, ["metadata", "refresh"]);
  assert.deepEqual(result, { status: "saved" });
});

test("loadModLibraryItemsForMode preserves the existing list on refresh failure", async () => {
  const result = await loadModLibraryItemsForMode({
    mode: "refresh",
    fallbackItems,
    getModLibrary: async () => {
      throw new Error("backend unavailable");
    },
    refreshInstallManifestStatuses: async (items) => items,
  });

  assert.equal(result.status, "unavailable");
  assert.equal(result.items, null);
});

test("loadModLibraryItemsForMode uses fallback data only for initial load failure", async () => {
  const result = await loadModLibraryItemsForMode({
    mode: "initial",
    fallbackItems,
    getModLibrary: async () => {
      throw new Error("backend unavailable");
    },
    refreshInstallManifestStatuses: async (items) => items,
  });

  assert.equal(result.status, "fallback");
  assert.equal(result.items, fallbackItems);
});

test("createDetailDialogState snapshots the opened item instead of tracking live library objects", () => {
  const state = createDetailDialogState("mod-a", fallbackItems);
  const refreshedItems = [{ ...fallbackItems[0], name: "Refreshed Alpha" }];

  assert.equal(state?.modId, "mod-a");
  assert.equal(state?.initialTab, "details");
  assert.equal(state?.fallbackItem?.name, "Alpha");
  assert.notEqual(state?.fallbackItem, refreshedItems[0]);

  const replacementState = createDetailDialogState("mod-a", fallbackItems, "replacement");
  assert.equal(replacementState.initialTab, "replacement");
});

test("preserveItemsOnRefreshFailure keeps current UI when refresh returns no real items", () => {
  assert.equal(preserveItemsOnRefreshFailure(fallbackItems, null), fallbackItems);
  assert.deepEqual(preserveItemsOnRefreshFailure(fallbackItems, []), []);
});

test("selectedIdsFromCategories uses loaded assignments before fallback labels", () => {
  const selected = selectedIdsFromCategories(
    [
      { id: "cat-a", name: "外观", sortOrder: 0, modCount: 1 },
      { id: "cat-b", name: "武器", sortOrder: 1, modCount: 1 },
    ],
    [{ id: "cat-b", name: "武器", sortOrder: 1 }],
    { id: "mod-a", name: "Alpha", status: "installed", sizeLabel: "12 MB", categoryLabels: [{ name: "外观" }] },
    true,
  );

  assert.deepEqual([...selected], ["cat-b"]);
});

test("parseNexusModId accepts empty values and positive integers only", () => {
  assert.equal(parseNexusModId(""), undefined);
  assert.equal(parseNexusModId(" 42 "), 42);
  assert.equal(parseNexusModId("0"), null);
  assert.equal(parseNexusModId("abc"), null);
});

test("getTrappedFocusIndex wraps keyboard focus inside the dialog", () => {
  assert.equal(getTrappedFocusIndex({ currentIndex: 0, focusableCount: 3, backwards: true }), 2);
  assert.equal(getTrappedFocusIndex({ currentIndex: 2, focusableCount: 3, backwards: false }), 0);
  assert.equal(getTrappedFocusIndex({ currentIndex: 1, focusableCount: 3, backwards: false }), null);
  assert.equal(getTrappedFocusIndex({ currentIndex: -1, focusableCount: 0, backwards: false }), -1);
});
