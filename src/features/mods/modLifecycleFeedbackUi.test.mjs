import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing lifecycle feedback source: ${path}`);
  return readFileSync(path, "utf8");
}

test("core lifecycle feedback uses the shared semantic surfaces", () => {
  const source = readSource("src/features/mods/ModLifecycleFeedback.tsx");

  assert.match(source, /DetailSheet/);
  assert.match(source, /Dialog/);
  assert.match(source, /TaskNotice/);
  assert.match(source, /TaskNoticeViewport/);
  assert.match(source, /ToastViewport/);
  assert.doesNotMatch(source, /createPortal|document\.body|position:\s*fixed/);
});

test("uninstall confirmation is an alert dialog with a safe close and focus policy", () => {
  const source = readSource("src/features/mods/ModLifecycleFeedback.tsx");
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /role="alertdialog"/);
  assert.match(source, /closeOnBackdrop=\{false\}/);
  assert.match(source, /initialFocusRef=\{cancelButtonRef\}/);
  assert.match(source, /ref=\{cancelButtonRef\}[^>]*>\s*取消/);
  assert.match(source, /disabled=\{blockerMessage\s*!==\s*null\}/);
  assert.match(source, /state\.managedFileCount/);
  assert.match(source, /state\.backupCount/);
  assert.match(page, /type PendingUninstallConfirmation[\s\S]*?profileId:\s*string/);
  assert.match(page, /activeProfileId\s*!==\s*uninstallConfirmation\.profileId/);
  assert.match(page, /currentSummary\.managedFileCount\s*===\s*uninstallConfirmation\.managedFileCount/);
  assert.match(page, /const \{ profileId, modId, modName \} = uninstallConfirmation/);
  assert.match(page, /startUninstallTask\([\s\S]*?profileId,/);
});

test("running notice is strictly task keyed and terminal toast stays feature local", () => {
  const source = readSource("src/features/mods/ModLifecycleFeedback.tsx");
  const stateSource = readSource("src/features/mods/modLifecycleFeedbackState.ts");

  assert.match(source, /taskState\.status\s*===\s*"running"/);
  assert.match(source, /taskId=\{runningTask\.taskId\}/);
  assert.match(source, /data-toast-id=\{toast\.id\}/);
  assert.match(source, /onDismissToast/);
  assert.doesNotMatch(source, /setTimeout|queue|dedupe/);
  assert.match(stateSource, /isPersistentRecoveryStatus\(refresh\.status\)/);
  assert.match(stateSource, /return null/);
});

test("terminal feedback is published only after durable manifest and recovery refresh", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const refreshStart = page.indexOf("const refreshTerminalFacts");
  const refreshCall = page.indexOf("await refreshInstallManifestStatusesWithOutcome", refreshStart);
  const identityGuard = page.indexOf("isManagedInstallTerminalRefreshCurrent", refreshCall);
  const toastCall = page.indexOf("setLifecycleToast(getManagedInstallTerminalToast", refreshStart);

  assert.ok(refreshStart >= 0);
  assert.ok(refreshCall > refreshStart);
  assert.ok(identityGuard > refreshCall);
  assert.ok(toastCall > identityGuard);
  assert.match(page, /failClosedModInstallSummary/);
  assert.match(page, /activeProfileIdRef\.current\s*!==\s*terminalTask\.profileId/);
  assert.match(page, /const libraryUnchanged = libraryItemsRef\.current === itemsAtRefreshStart/);
  assert.match(page, /setLifecycleToast\(null\)/);
  assert.match(page, /isManagedInstallTaskTerminal\(installTaskState\)/);
  assert.match(page, /handledInstallTerminalTaskIdsRef/);
});

test("closing the install sheet invalidates pending preview without cancelling a task", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const closeStart = page.indexOf("const closeInstallPlanDetail");
  const closeEnd = page.indexOf("};", closeStart);
  const closeBody = page.slice(closeStart, closeEnd);

  assert.match(closeBody, /installPlanPreviewGenerationRef\.current\s*\+=\s*1/);
  assert.match(closeBody, /setInstallPlanDetailState\(\{ status: "idle" \}\)/);
  assert.doesNotMatch(closeBody, /cancelTask|cancel_task|setTrackedInstallTaskState/);
});
