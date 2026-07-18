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
  const profileRefEffect = page.search(
    /useEffect\(\(\) => \{\s*activeProfileIdRef\.current = activeProfile\.status === "ready" \? activeProfileId : null;\s*\}, \[activeProfile\.status, activeProfileId\]\);/,
  );
  const durableProbeStart = page.indexOf("const refreshTerminalDurableStatus");
  const durableProbeEnd = page.indexOf("const reinstallWorkflow", durableProbeStart);
  const durableProbe = page.slice(durableProbeStart, durableProbeEnd);
  const refreshStart = page.indexOf("const refreshTerminalFacts");
  const scrollReset = page.indexOf("resetContentScroll();", refreshStart);
  const allSettledCall = page.indexOf("await Promise.allSettled", refreshStart);
  const pageRefreshCall = page.indexOf("refreshModLibrary()", allSettledCall);
  const durableRefreshCall = page.indexOf("refreshTerminalDurableStatus(", allSettledCall);
  const mountedGuard = page.indexOf("if (!pageMountedRef.current)", durableRefreshCall);
  const identityGuard = page.indexOf("currentProfileId !== terminalTask.profileId", mountedGuard);
  const terminalRefresh = page.indexOf("const terminalRefresh", identityGuard);
  const failClosedCall = page.indexOf("failClosedModInstallSummary(items, terminalTask.modId)", terminalRefresh);
  const toastCall = page.indexOf("setLifecycleToast(getManagedInstallTerminalToast", refreshStart);

  assert.ok(profileRefEffect >= 0);
  assert.ok(profileRefEffect < durableProbeStart);
  assert.equal(page.match(/activeProfileIdRef\.current\s*=/g)?.length, 1);
  assert.ok(durableProbeStart >= 0);
  assert.match(durableProbe, /refreshModLibraryDurableStatuses\(\[createModLibraryStatusProbe\(modId,\s*modName\)\]/);
  assert.match(durableProbe, /loadManifestStatuses:[\s\S]*?getInstallManifestStatus/);
  assert.match(durableProbe, /loadRecoveryStatuses:[\s\S]*?scanInstallRecovery/);
  assert.doesNotMatch(durableProbe, /libraryItems(?:Ref)?|libraryPage/);
  assert.ok(refreshStart >= 0);
  assert.ok(scrollReset > refreshStart);
  assert.ok(scrollReset < allSettledCall);
  assert.ok(allSettledCall > refreshStart);
  assert.ok(pageRefreshCall > allSettledCall);
  assert.ok(durableRefreshCall > pageRefreshCall);
  assert.ok(mountedGuard > durableRefreshCall);
  assert.ok(identityGuard > mountedGuard);
  assert.ok(terminalRefresh > identityGuard);
  assert.ok(failClosedCall > terminalRefresh);
  assert.ok(toastCall > failClosedCall);
  assert.match(page, /failClosedModInstallSummary/);
  assert.match(page, /activeProfileIdRef\.current\s*!==\s*terminalTask\.profileId/);
  assert.match(page, /durableRefresh\.status\s*===\s*"fulfilled"/);
  assert.match(page, /verified:\s*durableStatus\?\.verified\s*\?\?\s*false/);
  assert.match(page, /status:\s*durableStatus\?\.items\[0\]\?\.installSummary\?\.status\s*\?\?\s*null/);
  assert.match(page, /pageRefresh\.status\s*===\s*"rejected"/);
  assert.match(page, /shouldFailClosedManagedInstallTerminal\(terminalTask,\s*terminalRefresh\)/);
  assert.match(page, /setLifecycleToast\(null\)/);
  assert.match(page, /isManagedInstallTaskTerminal\(installTaskState\)/);
  assert.match(page, /handledInstallTerminalTaskIdsRef/);
});

test("query refresh blocks already-open uninstall and reinstall write confirmations", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(
    page,
    /const uninstallBlockerMessage = useMemo\(\(\) => \{[\s\S]*?if \(libraryQueryBusy\) \{\s*return MOD_LIBRARY_QUERY_BUSY_MESSAGE;/,
  );
  assert.match(
    page,
    /const startSelectedUninstallTask = \(\) => \{\s*if \(libraryQueryBusy \|\| !uninstallConfirmation \|\| uninstallBlockerMessage !== null\) \{\s*return;/,
  );
  assert.match(
    page,
    /const confirmSelectedReinstall = \(\) => \{\s*if \(libraryQueryBusy\) \{\s*return;\s*\}[\s\S]*?reinstallWorkflow\.confirmReinstall\(\);/,
  );
  assert.match(
    page,
    /<ReinstallPlanPreviewPanel[\s\S]*?canConfirm=\{reinstallWorkflow\.canConfirm && !libraryQueryBusy\}[\s\S]*?onConfirm=\{confirmSelectedReinstall\}/,
  );
});

test("reduced motion disables lifecycle feedback animations", () => {
  const css = readSource("src/features/mods/ModLifecycleFeedback.css");
  const reducedMotionBlock = css.match(
    /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{([\s\S]*?)\n\}/,
  )?.[1];

  assert.ok(reducedMotionBlock);
  assert.match(reducedMotionBlock, /\.mod-lifecycle-feedback__spinner/);
  assert.match(reducedMotionBlock, /\.mod-lifecycle-feedback__task-progress span/);
  assert.match(reducedMotionBlock, /animation:\s*none/);
  assert.doesNotMatch(reducedMotionBlock, /animation-duration/);
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
