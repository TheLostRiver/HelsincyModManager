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
  assert.match(panel, /batchSelectionActive \? compact\.exitBatch : compact\.enterBatch/);
  assert.match(panel, /"exit-batch-selection" : "enter-batch-selection"/);
  assert.match(panel, /\.filter\(\(a\) => \["select-all", "invert", "refresh"\]\.includes\(a\.id\)\)/);
  assert.match(panel, /aria-label=\{compact\.clearSelectionAria\}/);
  // I18N-02 起批量文案钉在 modLibraryCopy 的 zh_cn 字典。
  const copySource = readSource("src/features/mods/modLibraryCopy.ts");
  assert.match(panel, /"preview-plan": compact\.batchActionLabels\.previewPlan/);
  assert.match(panel, /install: compact\.batchActionLabels\.install/);
  assert.match(panel, /reinstall: compact\.batchActionLabels\.reinstall/);
  assert.match(panel, /uninstall: compact\.batchActionLabels\.uninstall/);
  assert.match(copySource, /预览批量计划/);
  assert.match(copySource, /批量安装/);
  assert.match(
    panel,
    /compact\.selectedSummary\(selectedCount, MAX_MOD_SELECTION_COUNT, selectedPageCount, pageCount\)/,
  );
  assert.match(copySource, /已选 \$\{selected\} \/ \$\{max\}，本页已选 \$\{pageSelected\} \/ \$\{pageCount\} 项/);

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
  assert.match(panel, /compact\.footerSinglePage\(selectedPageCount, pageCount\)/);
  assert.match(page, /case "refresh":\s*if \(!selectionInteractionLocked\) \{\s*void refreshModLibrary\(\)/);
});

test("batch lifecycle actions honor workflow lock and backend capability before dispatch", () => {
  const panel = readSource("src/features/mods/CompactActionPanel.tsx");
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(
    panel,
    /const lifecycleDisabledReason = \([\s\S]*?selectionInteractionDisabledReason\s*\?\?\s*batchCapabilityDisabledReason\(actionId\)\s*\?\?\s*fallbackReason/,
  );
  assert.match(panel, /batchSelectionActive && actionId === "preview-plan"/);
  assert.match(panel, /batchPreviewUnavailableReason/);
  assert.match(panel, /batchWriteUnavailableReason/);

  assert.match(page, /const batchCapability = useBatchModLifecycleCapability\(\)/);
  assert.match(page, /batchWriteAvailable = batchCapability\.capability\?\.writeAvailable === true/);
  assert.match(page, /batchPreviewAvailable = batchCapability\.capability\?\.previewAvailable === true/);
  assert.match(
    page,
    /selectedIds\.size > 0 && batchWriteAvailable/,
  );
  assert.match(
    page,
    /case "preview-plan":[\s\S]*?else if \(!selectionInteractionLocked && batchPreviewAvailable\)/,
  );
  assert.match(
    page,
    /case "install":[\s\S]*?else if \(!selectionInteractionLocked && batchWriteAvailable\)/,
  );
  assert.match(
    page,
    /case "reinstall":[\s\S]*?else if \(!selectionInteractionLocked && batchWriteAvailable\)/,
  );
  assert.match(
    page,
    /case "uninstall":[\s\S]*?else if \(!selectionInteractionLocked && batchWriteAvailable\)/,
  );
  assert.match(page, /batchPreviewUnavailableReason=\{batchPreviewUnavailableReason\}/);
  assert.match(page, /batchWriteUnavailableReason=\{batchWriteUnavailableReason\}/);
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
  assert.match(page, /selectionMode === "batch"[\s\S]*?copy\.page\.cardAction\.batchSelecting/);
  assert.match(menu, /disabled=\{lifecycleDisabled\}/);
});

test("context menu lifecycle action reuses the existing single-item install and uninstall workflows", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const menu = readSource("src/features/mods/ModContextMenu.tsx");

  assert.match(page, /status === "installed"[\s\S]*?copy\.page\.cardAction\.uninstallLabel/);
  assert.match(page, /status === "not_installed"[\s\S]*?copy\.page\.cardAction\.installLabel/);
  assert.match(page, /case "install":\s*startSelectedInstallTask\(modId\)/);
  assert.match(page, /case "uninstall":\s*promptSelectedUninstallTask\(modId\)/);
  assert.match(page, /const startSelectedInstallTask = \(requestedModId\?: string\)/);
  assert.match(page, /const promptSelectedUninstallTask = \(requestedModId\?: string\)/);
  assert.match(page, /selectionInteractionLocked/);
  assert.match(menu, /handleItemClick\(resolvedLifecycleAction\.actionId\)/);
  assert.doesNotMatch(menu, /toggle-enable|启用 \/ 禁用/);
});

test("query context changes reset selection while pagination keeps it", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(page, /resetPageInteraction\("search-changed"\)/);
  assert.match(page, /resetPageInteraction\("filters-changed"\)/);
  assert.match(page, /dispatchSelection\(\{ type: "reset-context", reason: "profile-changed" \}\)/);
  assert.match(page, /dispatchSelection\(\{ type: "reset-context", reason: "library-refreshed" \}\)/);
  assert.match(page, /const handlePageChange[\s\S]*?libraryQuery\.setPage\(nextPage\);\s*resetContentScroll\(\);/);
  assert.match(page, /const handlePageSizeChange[\s\S]*?libraryQuery\.setPageSize\(nextPageSize\);\s*resetContentScroll\(\);/);
});
