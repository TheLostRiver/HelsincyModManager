import assert from "node:assert/strict";
import { test } from "node:test";

import {
  allLibraryFilter,
  buildLibraryFilterChips,
  libraryFilterKey,
  matchesLibraryFilter,
  normalizeLibraryFilter,
  visibleCategoryLabelsForCard,
} from "./modLibraryFilters.ts";

const installedItem = {
  id: "installed-mod",
  name: "Installed Mod",
  sizeLabel: "12 MB",
  status: "installed",
  categoryLabels: [{ name: "已安装", color: "#ef4444" }],
};

const weaponItem = {
  id: "weapon-mod",
  name: "Weapon Mod",
  sizeLabel: "8 MB",
  status: "not_installed",
  categoryLabels: [{ name: "武器", color: "#2563eb" }],
};

test("buildLibraryFilterChips keeps status filters and appends non-empty categories in sort order", () => {
  const chips = buildLibraryFilterChips([
    { id: "cat-empty", name: "空分类", color: "#94a3b8", sortOrder: 1, modCount: 0 },
    { id: "cat-weapon", name: "武器", color: "#2563eb", sortOrder: 5, modCount: 2 },
    { id: "cat-armor", name: "外观", color: "#db2777", sortOrder: 2, modCount: 4 },
  ]);

  assert.deepEqual(
    chips.map((chip) => chip.label),
    ["全部", "已安装", "未安装", "外观", "武器"],
  );
  assert.deepEqual(chips.slice(3).map((chip) => chip.color), ["#db2777", "#2563eb"]);
});

test("status filters stay selected but disabled without an active profile", () => {
  const chips = buildLibraryFilterChips([], {
    statusFiltersEnabled: false,
    statusDisabledReason: "选择配置档后可用",
  });
  const statusChips = chips.filter((chip) => chip.kind === "status");

  assert.deepEqual(statusChips.map((chip) => chip.label), ["已安装", "未安装"]);
  assert.ok(statusChips.every((chip) => chip.disabled));
  assert.ok(statusChips.every((chip) => chip.disabledReason === "选择配置档后可用"));
  assert.deepEqual(
    normalizeLibraryFilter({ kind: "status", status: "installed" }, chips),
    { kind: "status", status: "installed" },
  );
});

test("library category filters do not collide with status labels of the same name", () => {
  const chips = buildLibraryFilterChips([
    { id: "cat-installed", name: "已安装", color: "#ef4444", sortOrder: 0, modCount: 1 },
  ]);
  const statusChip = chips.find((chip) => chip.kind === "status" && chip.label === "已安装");
  const categoryChip = chips.find((chip) => chip.kind === "category" && chip.label === "已安装");

  assert.ok(statusChip);
  assert.ok(categoryChip);
  assert.notEqual(libraryFilterKey(statusChip.filter), libraryFilterKey(categoryChip.filter));
  assert.equal(matchesLibraryFilter(installedItem, statusChip.filter), true);
  assert.equal(matchesLibraryFilter(weaponItem, statusChip.filter), false);
  assert.equal(matchesLibraryFilter(installedItem, categoryChip.filter), true);
  assert.equal(matchesLibraryFilter(weaponItem, categoryChip.filter), false);
});

test("normalizeLibraryFilter refreshes renamed category filters from the current chip", () => {
  const chips = buildLibraryFilterChips([
    { id: "cat-weapons", name: "Weapons", color: "#2563eb", sortOrder: 0, modCount: 1 },
  ]);
  const staleFilter = {
    kind: "category",
    categoryId: "cat-weapons",
    categoryName: "Old Weapons",
  };

  const normalized = normalizeLibraryFilter(staleFilter, chips);

  assert.deepEqual(normalized, {
    kind: "category",
    categoryId: "cat-weapons",
    categoryName: "Weapons",
  });
});

test("normalizeLibraryFilter preserves references when rebuilt chips are semantically unchanged", () => {
  const statusFilter = { kind: "status", status: "installed" };
  const categoryFilter = {
    kind: "category",
    categoryId: "cat-weapons",
    categoryName: "Weapons",
  };
  const chips = buildLibraryFilterChips([
    { id: "cat-weapons", name: "Weapons", color: "#2563eb", sortOrder: 0, modCount: 1 },
  ]);

  assert.strictEqual(normalizeLibraryFilter(statusFilter, chips), statusFilter);
  assert.strictEqual(normalizeLibraryFilter(categoryFilter, chips), categoryFilter);
  assert.strictEqual(normalizeLibraryFilter(allLibraryFilter, chips), allLibraryFilter);
});

test("visibleCategoryLabelsForCard limits visible labels and reports overflow count", () => {
  const result = visibleCategoryLabelsForCard(
    [
      { name: "外观", color: "#db2777" },
      { name: "武器", color: "#2563eb" },
      { name: "语音", color: "#16a34a" },
      { name: "工具", color: "#f59e0b" },
    ],
    3,
  );

  assert.deepEqual(result.visible.map((label) => label.name), ["外观", "武器", "语音"]);
  assert.equal(result.overflowCount, 1);
});

test("visibleCategoryLabelsForCard hides empty label strips", () => {
  const result = visibleCategoryLabelsForCard([], 3);

  assert.deepEqual(result.visible, []);
  assert.equal(result.overflowCount, 0);
});
