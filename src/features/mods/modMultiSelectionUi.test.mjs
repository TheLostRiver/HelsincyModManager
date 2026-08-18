import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("compact actions expose an explicit persistent batch mode", () => {
  const panel = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(panel, /ListChecks/);
  assert.match(panel, /aria-pressed=\{batchSelectionActive\}/);
  assert.match(panel, /batchSelectionActive \? "退出批量选择" : "批量选择"/);
  assert.match(panel, /"exit-batch-selection" : "enter-batch-selection"/);
  assert.match(panel, /\.filter\(\(a\) => \["select-all", "invert", "refresh"\]\.includes\(a\.id\)\)/);
  assert.match(panel, /aria-label="清空选择"/);
  assert.match(panel, /"preview-plan": "预览批量计划"/);
  assert.match(panel, /install: "批量安装"/);
  assert.match(panel, /reinstall: "批量重装"/);
  assert.match(panel, /uninstall: "批量卸载"/);
  assert.match(panel, /已选 \{selectedCount\} \/ \{MAX_MOD_SELECTION_COUNT\}/);
  assert.match(panel, /本页已选 \{selectedPageCount\} \/ \{pageCount\} 项/);

  const pageSelectionGroupIndex = panel.indexOf('<div className="compact-action-group">');
  const batchModeToggleIndex = panel.indexOf('className="compact-action is-neutral is-mode-toggle"');
  assert.ok(pageSelectionGroupIndex >= 0);
  assert.ok(batchModeToggleIndex > pageSelectionGroupIndex);
  assert.doesNotMatch(panel, /batchSelectionActive \? <div className="compact-action-group">/);
});

test("refresh remains visible but honors the workflow selection lock and page-local count", () => {
  const panel = readSource("src/features/mods/CompactActionPanel.tsx");
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(panel, /const disabledReason = selectionInteractionDisabledReason\s*\n\s*\?\? getCompactActionDisabledReason/);
  assert.match(panel, /本页已选 \{selectedPageCount\} \/ 当前页 \{pageCount\} 项/);
  assert.match(page, /case "refresh":\s*if \(!selectionInteractionLocked\) \{\s*void refreshModLibrary\(\)/);
});

test("mod cards convert pointer and keyboard Ctrl input into pure selection intents", () => {
  const card = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(card, /onSelect: \(intent: ModCardSelectionIntent\) => void/);
  assert.match(card, /selectWithIntent\(event\.ctrlKey, event\.ctrlKey \? "ctrl-pointer" : "pointer"\)/);
  assert.match(card, /selectWithIntent\(e\.ctrlKey, e\.ctrlKey \? "ctrl-keyboard" : "keyboard"\)/);
  assert.match(card, /role=\{batchSelectionActive \? "checkbox" : "button"\}/);
  assert.match(card, /aria-pressed=\{batchSelectionActive \? undefined : selected\}/);
  assert.match(card, /aria-checked=\{batchSelectionActive \? selected : undefined\}/);
  assert.match(card, /data-selection-mode=\{selectionMode\}/);
});

test("all card views keep an explicit fixed selection indicator", () => {
  const card = readSource("src/features/mods/ModPosterCard.tsx");
  const css = readSource("src/features/mods/ModPosterCard.css");

  assert.match(card, /<div className="mod-card__selection-indicator" aria-hidden="true">/);
  assert.doesNotMatch(card, /!isClassic &&/);
  assert.match(css, /\.mod-card\.is-batch-selection \.mod-card__selection-indicator\s*{[\s\S]*?opacity:\s*1;/);
  assert.match(css, /\.mod-grid\.view-tech \.mod-card__selection-indicator\s*{[\s\S]*?display:\s*flex;/);
});

test("batch mode keeps right-click selection intact and disables single-item writes", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const menu = readSource("src/features/mods/ModContextMenu.tsx");

  assert.match(page, /if \(selectionMode === "single" && !selectedIds\.has\(modId\)\)/);
  assert.match(page, /batchSelectionActive=\{selectionMode === "batch"\}/);
  assert.match(menu, /aria-disabled=\{batchSelectionActive \|\| undefined\}/);
  assert.match(menu, /批量选择中，请使用上方批量操作/);
});

test("query context changes reset selection while pagination keeps it", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(page, /resetPageInteraction\("搜索条件已变化"\)/);
  assert.match(page, /resetPageInteraction\("筛选条件已变化"\)/);
  assert.match(page, /dispatchSelection\(\{ type: "reset-context", reason: "活动配置档已变化" \}\)/);
  assert.match(page, /dispatchSelection\(\{ type: "reset-context", reason: "Mod 库已刷新" \}\)/);
  assert.match(page, /const handlePageChange[\s\S]*?libraryQuery\.setPage\(nextPage\);\s*resetContentScroll\(\);/);
  assert.match(page, /const handlePageSizeChange[\s\S]*?libraryQuery\.setPageSize\(nextPageSize\);\s*resetContentScroll\(\);/);
});
