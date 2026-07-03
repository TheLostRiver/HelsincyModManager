import assert from "node:assert/strict";
import { test } from "node:test";

import {
  buildSortOrderUpdates,
  canReorderCategories,
  filterCategories,
  findDuplicateCategoryName,
  getCategoryMetrics,
  getCategoryMutationErrorMessage,
  moveCategoryByOffset,
  nextAppendSortOrder,
  pruneCategorySelection,
  reorderCategoryList,
  sortCategoriesForView,
  summarizeBatchTargets,
  toggleCategorySelection,
} from "./categoryWorkflow.ts";

function cat(id, name, sortOrder, modCount = 0, color) {
  return { id, name, color, sortOrder, modCount };
}

const fixtures = [
  cat("a", "外观", 0, 12, "#DB2777"),
  cat("b", "武器替换", 1, 8, "#2563EB"),
  cat("c", "语音替换", 2, 0, "#7C3AED"),
  cat("d", "工具", 3, 3),
];

test("getCategoryMetrics aggregates totals, linked mods, empty and colored counts", () => {
  assert.deepEqual(getCategoryMetrics(fixtures), {
    total: 4,
    linkedModCount: 23,
    emptyCategoryCount: 1,
    coloredCategoryCount: 3,
  });
  assert.deepEqual(getCategoryMetrics([]), {
    total: 0,
    linkedModCount: 0,
    emptyCategoryCount: 0,
    coloredCategoryCount: 0,
  });
});

test("filterCategories matches by name case-insensitively and ignores blank queries", () => {
  assert.deepEqual(filterCategories(fixtures, "  ").map((c) => c.id), ["a", "b", "c", "d"]);
  assert.deepEqual(filterCategories(fixtures, "替换").map((c) => c.id), ["b", "c"]);
  const latin = [cat("x", "Armor Pack", 0), cat("y", "voice", 1)];
  assert.deepEqual(filterCategories(latin, "ARMOR").map((c) => c.id), ["x"]);
  assert.deepEqual(filterCategories(fixtures, "不存在"), []);
});

test("sortCategoriesForView supports custom, name and modCount views without mutating input", () => {
  const shuffled = [fixtures[2], fixtures[0], fixtures[3], fixtures[1]];
  assert.deepEqual(sortCategoriesForView(shuffled, "custom").map((c) => c.id), ["a", "b", "c", "d"]);
  assert.deepEqual(sortCategoriesForView(shuffled, "modCount").map((c) => c.id), ["a", "b", "d", "c"]);
  assert.deepEqual(sortCategoriesForView(shuffled, "name").map((c) => c.name)[0], "工具");
  assert.deepEqual(shuffled.map((c) => c.id), ["c", "a", "d", "b"], "input must stay untouched");
});

test("sortCategoriesForView custom view breaks sortOrder ties by name", () => {
  const tied = [cat("x", "乙", 5), cat("y", "甲", 5)];
  const sorted = sortCategoriesForView(tied, "custom");
  assert.equal(sorted[0].name.localeCompare(sorted[1].name, "zh-Hans-CN") <= 0, true);
});

test("canReorderCategories only allows custom view without query or batch mode", () => {
  assert.equal(canReorderCategories("custom", "", false), true);
  assert.equal(canReorderCategories("custom", "  ", false), true);
  assert.equal(canReorderCategories("custom", "武器", false), false);
  assert.equal(canReorderCategories("custom", "", true), false);
  assert.equal(canReorderCategories("name", "", false), false);
  assert.equal(canReorderCategories("modCount", "", false), false);
});

test("reorderCategoryList moves items to the insertion index and rejects no-op moves", () => {
  assert.deepEqual(reorderCategoryList(fixtures, 0, 3).map((c) => c.id), ["b", "c", "a", "d"]);
  assert.deepEqual(reorderCategoryList(fixtures, 3, 0).map((c) => c.id), ["d", "a", "b", "c"]);
  assert.deepEqual(reorderCategoryList(fixtures, 1, 4).map((c) => c.id), ["a", "c", "d", "b"]);
  assert.equal(reorderCategoryList(fixtures, 1, 1), null);
  assert.equal(reorderCategoryList(fixtures, 1, 2), null);
  assert.equal(reorderCategoryList(fixtures, -1, 0), null);
  assert.equal(reorderCategoryList(fixtures, 0, 5), null);
});

test("moveCategoryByOffset moves one position and stops at the edges", () => {
  assert.deepEqual(moveCategoryByOffset(fixtures, "b", -1).map((c) => c.id), ["b", "a", "c", "d"]);
  assert.deepEqual(moveCategoryByOffset(fixtures, "b", 1).map((c) => c.id), ["a", "c", "b", "d"]);
  assert.equal(moveCategoryByOffset(fixtures, "a", -1), null);
  assert.equal(moveCategoryByOffset(fixtures, "d", 1), null);
  assert.equal(moveCategoryByOffset(fixtures, "missing", 1), null);
});

test("buildSortOrderUpdates renumbers by display index and only emits changed rows", () => {
  const reordered = reorderCategoryList(fixtures, 3, 0);
  assert.deepEqual(buildSortOrderUpdates(reordered), [
    { categoryId: "d", sortOrder: 0 },
    { categoryId: "a", sortOrder: 1 },
    { categoryId: "b", sortOrder: 2 },
    { categoryId: "c", sortOrder: 3 },
  ]);
  assert.deepEqual(buildSortOrderUpdates(fixtures), []);

  const sparse = [cat("a", "外观", 10), cat("b", "武器", 20)];
  assert.deepEqual(buildSortOrderUpdates(sparse), [
    { categoryId: "a", sortOrder: 0 },
    { categoryId: "b", sortOrder: 1 },
  ]);
});

test("nextAppendSortOrder appends after the current maximum", () => {
  assert.equal(nextAppendSortOrder([]), 0);
  assert.equal(nextAppendSortOrder(fixtures), 4);
  assert.equal(nextAppendSortOrder([cat("a", "外观", 40), cat("b", "武器", 7)]), 41);
});

test("findDuplicateCategoryName trims, ignores case and can exclude the edited row", () => {
  assert.equal(findDuplicateCategoryName(fixtures, " 外观 ").id, "a");
  assert.equal(findDuplicateCategoryName(fixtures, "外观", "a"), undefined);
  assert.equal(findDuplicateCategoryName(fixtures, ""), undefined);
  const latin = [cat("x", "Armor", 0)];
  assert.equal(findDuplicateCategoryName(latin, "armor").id, "x");
});

test("selection helpers toggle, prune and summarize batch targets", () => {
  const selected = toggleCategorySelection(new Set(), "a");
  assert.deepEqual([...selected], ["a"]);
  assert.deepEqual([...toggleCategorySelection(selected, "a")], []);

  const pruned = pruneCategorySelection(new Set(["a", "ghost"]), fixtures);
  assert.deepEqual([...pruned], ["a"]);

  assert.deepEqual(summarizeBatchTargets(fixtures, new Set(["a", "c"])), {
    count: 2,
    linkedModCount: 12,
  });
  assert.deepEqual(summarizeBatchTargets(fixtures, new Set()), { count: 0, linkedModCount: 0 });
});

test("getCategoryMutationErrorMessage hides backend command errors behind the fallback", () => {
  assert.equal(
    getCategoryMutationErrorMessage({ code: "category_error", message: "raw db failure" }, "回退文案"),
    "回退文案",
  );
  assert.equal(getCategoryMutationErrorMessage(new Error("网络中断"), "回退文案"), "网络中断");
  assert.equal(getCategoryMutationErrorMessage("直接字符串", "回退文案"), "直接字符串");
  assert.equal(getCategoryMutationErrorMessage(undefined, "回退文案"), "回退文案");
  assert.equal(getCategoryMutationErrorMessage({ message: "  " }, "回退文案"), "回退文案");
});
