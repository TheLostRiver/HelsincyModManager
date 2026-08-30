import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing mod delete source: ${path}`);
  return readFileSync(path, "utf8");
}

function sliceDeleteDialog(feedback) {
  const start = feedback.indexOf("export function DeleteConfirmationDialog(");
  const end = feedback.indexOf("type ManagedInstallTaskFeedbackProps", start);
  assert.ok(start >= 0, "ModLifecycleFeedback must export DeleteConfirmationDialog");
  assert.ok(end > start, "DeleteConfirmationDialog body must precede the task feedback props");
  return feedback.slice(start, end);
}

function slicePageBlock(page, startNeedle, endNeedle) {
  const start = page.indexOf(startNeedle);
  const end = page.indexOf(endNeedle, start);
  assert.ok(start >= 0, `missing page block: ${startNeedle}`);
  assert.ok(end > start, `unterminated page block: ${startNeedle}`);
  return page.slice(start, end);
}

test("delete confirmation is an alert dialog with a safe close and focus policy", () => {
  const dialog = sliceDeleteDialog(readSource("src/features/mods/ModLifecycleFeedback.tsx"));

  assert.match(dialog, /role="alertdialog"/);
  assert.match(dialog, /closeOnBackdrop=\{false\}/);
  assert.match(dialog, /initialFocusRef=\{cancelButtonRef\}/);
  assert.match(dialog, /ref=\{cancelButtonRef\}[\s\S]*?\{deleteCopy\.cancel\}/);
  // The destructive button stays disabled while the deletion is in flight and when
  // every listed entry was skipped, so a confirm can never fire without a target.
  assert.match(dialog, /disabled=\{busy \|\| pending\.length === 0\}/);
  assert.match(dialog, /busy \? deleteCopy\.confirmBusy : deleteCopy\.confirm/);
  assert.match(dialog, /const pending = state\.mods\.filter\(\(entry\) => entry\.skip !== true\)/);
  assert.match(dialog, /mod-lifecycle-feedback__audit-note/);
});

test("single confirmation shows backend preview facts, batch confirmation shows the roster", () => {
  const dialog = sliceDeleteDialog(readSource("src/features/mods/ModLifecycleFeedback.tsx"));

  assert.match(dialog, /const batch = state\.mods\.length > 1/);
  assert.match(dialog, /title=\{batch \? deleteCopy\.batchTitle : deleteCopy\.singleTitle\}/);
  assert.match(dialog, /deleteCopy\.metricRevisions/);
  assert.match(dialog, /deleteCopy\.metricCategories/);
  assert.match(dialog, /primary\.affectedProfiles/);
  assert.match(dialog, /deleteCopy\.affectedProfilesEmpty/);
  assert.match(dialog, /mod-lifecycle-feedback__delete-list/);
  assert.match(dialog, /\{state\.mods\.map\(\(entry\) => \(/);
  assert.match(dialog, /key=\{entry\.modId\}/);
  assert.match(dialog, /entry\.skipReason \? <em>\{entry\.skipReason\}<\/em> : null/);
  // The dialog must never invent install state: it only projects what the backend returned.
  assert.doesNotMatch(dialog, /installSummary|scanInstallRecovery/);
});

test("card context menu exposes a guarded delete entry", () => {
  const menu = readSource("src/features/mods/ModContextMenu.tsx");

  assert.match(menu, /deleteAction\?: \{/);
  assert.match(menu, /deleteAction \? \(/);
  assert.match(menu, /mod-context-menu__item is-danger/);
  assert.match(menu, /aria-disabled=\{deleteAction\.disabledReason !== undefined\}/);
  assert.match(menu, /disabled=\{deleteAction\.disabledReason !== undefined\}/);
  assert.match(menu, /handleItemClick\("delete"\)/);
  assert.match(menu, /\{deleteAction\.disabledReason \? <small>\{deleteAction\.disabledReason\}<\/small> : null\}/);
});

test("library page gates deletion on the current-profile install view before any backend call", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const prompt = slicePageBlock(
    page,
    "const promptDeleteMods =",
    "const cancelDeleteConfirmation",
  );

  assert.match(prompt, /libraryQueryBusy/);
  assert.match(prompt, /selectionInteractionLocked/);
  assert.match(prompt, /deletionBusy/);
  assert.match(prompt, /activeProfileId === null/);
  assert.match(prompt, /item\?\.installSummary\?\.status !== "not_installed"/);
  assert.match(prompt, /skipReason: deleteCopy\.dialog\.skipInstalled/);
  assert.match(prompt, /await previewModDeletion\(modId\)/);
  assert.match(prompt, /skipReason: deleteCopy\.dialog\.skipPreviewUnavailable/);
  assert.match(prompt, /setDeleteConfirmation\(\{ mods: entries \}\)/);
  // A stale preview must not overwrite state after the page unmounts.
  assert.match(prompt, /if \(!pageMountedRef\.current\)/);
});

test("confirmation deletes one mod at a time and maps backend error codes", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const confirm = slicePageBlock(page, "const confirmDeleteMods =", "const handleAction =");

  assert.match(confirm, /const targets = deleteConfirmation\.mods\.filter\(\(entry\) => entry\.skip !== true\)/);
  assert.match(confirm, /setDeletionBusy\(true\)/);
  assert.match(confirm, /for \(const entry of targets\)/);
  assert.match(confirm, /await deleteModFromLibrary\(entry\.modId\)/);
  assert.match(confirm, /deleteCopy\.errors\.codes\[code\]/);
  assert.match(confirm, /deleteCopy\.errors\.fallback/);
  assert.match(confirm, /toastCopy\.deletedTitle\(deleted\)/);
  assert.match(confirm, /toastCopy\.deleteFailedTitle\(failures\.length\)/);
  assert.match(confirm, /setDeleteConfirmation\(null\)/);
  assert.match(confirm, /await refreshModLibraryAfterWrite\(\)/);
  // Batch deletion v1 is a page-side loop over the single delete command, not a new
  // batch lifecycle operation.
  assert.doesNotMatch(confirm, /batchWorkflow\.prepare/);
});

test("library page routes single, batch and context menu entries into one confirmation", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(page, /<DeleteConfirmationDialog/);
  assert.match(page, /state=\{deleteConfirmation\}/);
  assert.match(page, /busy=\{deletionBusy\}/);
  assert.match(page, /onCancel=\{cancelDeleteConfirmation\}/);
  assert.match(page, /deleteAction=\{contextMenuDeleteAction\}/);
  assert.match(
    page,
    /canDeleteSelection=\{selectionMode === "batch" && batchWriteUnavailableReason === undefined\}/,
  );
  assert.match(page, /case "delete":\s*void promptDeleteMods\(\[modId\]\);/);

  const handleAction = slicePageBlock(page, "const handleAction =", "const handleContextMenuAction");
  assert.match(handleAction, /void promptDeleteMods\(\[selectedItem\.id\]\)/);
  assert.match(handleAction, /void promptDeleteMods\(Array\.from\(selectedIds\)\)/);

  const menuAction = slicePageBlock(
    page,
    "const contextMenuDeleteAction = useMemo",
    "const handleQueryChange",
  );
  assert.match(menuAction, /deleteCopy\.menu\.delete/);
  assert.match(menuAction, /deleteCopy\.menu\.deleteBlockedInstalled/);
  assert.match(menuAction, /copy\.page\.cardAction\.batchSelecting/);
});

test("compact panel exposes delete as a batch-only action", () => {
  const data = readSource("src/features/mods/modsLibraryData.ts");
  const panel = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(data, /\{ id: "delete", /);
  // Single deletion lives in the card context menu; rendering this button outside batch
  // selection would leave a permanently disabled control.
  assert.match(panel, /\.filter\(\(a\) => a\.id !== "delete" \|\| batchSelectionActive\)/);
});

test("delete copy covers every stable backend error code in all three locales", () => {
  const copy = readSource("src/features/mods/modDeleteCopy.ts");

  for (const code of [
    "mod_delete_blocked_installed",
    "mod_delete_blocked_recovery",
    "mod_delete_target_not_found",
    "mod_delete_store_unavailable",
  ]) {
    const occurrences = copy.match(new RegExp(code, "g")) ?? [];
    assert.equal(occurrences.length, 3, `${code} must be defined in zh_cn, en and ja`);
  }

  assert.match(copy, /satisfies LocaleDictionary<ModDeleteCopy>/);
  assert.match(copy, /skipPreviewUnavailable/);
});
