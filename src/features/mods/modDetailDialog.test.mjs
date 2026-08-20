import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { getTrappedFocusIndex } from "../../shared/feedback/focusTrap.ts";
import {
  createDetailDialogState,
  loadModLibraryItemsForMode,
  preserveItemsOnRefreshFailure,
} from "./modLibraryRefresh.ts";
import {
  formFieldFromOptional,
  formFromDetail,
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

test("formFieldFromOptional turns both null and undefined into an empty field", () => {
  // 后端 Option<u64> 序列化成 null，不是缺席字段；曾经的 === undefined 判断让表单显示 "null"。
  assert.equal(formFieldFromOptional(null), "");
  assert.equal(formFieldFromOptional(undefined), "");
  assert.equal(formFieldFromOptional(0), "0");
  assert.equal(formFieldFromOptional(42), "42");
  assert.equal(formFieldFromOptional(""), "");
});

test("formFromDetail leaves an absent NexusMods ID blank instead of literal null", () => {
  const detail = {
    id: "mod-a",
    name: "Alpha",
    packageId: "pkg-a",
    metadata: { tags: [], dependencies: [] },
    nexusModId: null,
  };

  const form = formFromDetail(detail, null);
  assert.equal(form.nexusModId, "");
  // 空值必须能原样存回：parseNexusModId("") 是合法的"未填写"，"null" 会被判成非法输入。
  assert.equal(parseNexusModId(form.nexusModId), undefined);
});

test("formFromDetail keeps a real NexusMods ID round-trippable", () => {
  const form = formFromDetail(
    {
      id: "mod-a",
      name: "Alpha",
      packageId: "pkg-a",
      metadata: { tags: [], dependencies: [] },
      nexusModId: 42,
    },
    null,
  );

  assert.equal(form.nexusModId, "42");
  assert.equal(parseNexusModId(form.nexusModId), 42);
});

test("getTrappedFocusIndex wraps keyboard focus inside the dialog", () => {
  assert.equal(getTrappedFocusIndex({ currentIndex: 0, focusableCount: 3, backwards: true }), 2);
  assert.equal(getTrappedFocusIndex({ currentIndex: 2, focusableCount: 3, backwards: false }), 0);
  assert.equal(getTrappedFocusIndex({ currentIndex: 1, focusableCount: 3, backwards: false }), null);
  assert.equal(getTrappedFocusIndex({ currentIndex: -1, focusableCount: 0, backwards: false }), -1);
});

test("dialog exit animation duration stays in sync with the component's close delay", () => {
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
  const read = (relativePath) => readFileSync(join(repoRoot, relativePath), "utf8");
  const component = read("src/features/mods/ModDetailDialog.tsx");
  const css = read("src/features/mods/ModDetailDialog.css");

  /*
   * 退场是"先标记、等动画播完再卸载"两段式：组件按常量延迟才调用 onClose，
   * CSS 负责这段时间内的动画。两者不一致会让对话框先消失再空等，或动画被卸载打断。
   * 这条契约没有类型或运行时保护，因此显式锁定。
   */
  const exitDurationMs = Number(component.match(/const DIALOG_EXIT_DURATION_MS = (\d+);/)?.[1]);
  assert.ok(Number.isInteger(exitDurationMs) && exitDurationMs > 0, "缺少 DIALOG_EXIT_DURATION_MS 常量");

  const exitRules = [...css.matchAll(/\.mod-detail-dialog__backdrop\.is-exiting[^{]*\{([\s\S]*?)\}/g)];
  assert.ok(exitRules.length > 0, "缺少退场规则");
  for (const [, body] of exitRules) {
    const declaredMs = body.match(/animation:[^;]*?(\d+)ms/)?.[1];
    if (declaredMs === undefined) {
      continue;
    }
    assert.equal(Number(declaredMs), exitDurationMs, `退场动画 ${declaredMs}ms 与组件常量 ${exitDurationMs}ms 不一致`);
  }

  // 所有关闭入口都必须走 requestClose，否则那条路径不会播退场动画。
  assert.match(component, /const requestClose = useCallback/);
  assert.match(component, /onRequestClose: requestClose/);
  assert.equal(component.match(/onClick=\{requestClose\}/g)?.length, 3);
  assert.equal(component.match(/onClose\(\)/g)?.length, 1);
  // 退场期间屏蔽交互，避免点到正在消失的控件。
  assert.match(css, /\.mod-detail-dialog__backdrop\.is-exiting\s*\{[\s\S]*?pointer-events:\s*none/);
});
