import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("install plan API invokes backend-driven imported mod preview command", () => {
  assert.equal(existsSync("src/features/mods/modInstallPlanApi.ts"), true);
  const source = readSource("src/features/mods/modInstallPlanApi.ts");

  assert.match(source, /invoke<InstallPlanPreview>\("preview_imported_mod_install_plan"/);
  assert.match(source, /preview_imported_mod_install_plan"[\s\S]*request:\s*\{/);
  assert.match(source, /gameId:\s*input\.gameId/);
  assert.match(source, /modId:\s*input\.modId/);
  assert.match(source, /layerName:\s*input\.layerName/);
  assert.match(source, /layerPriority:\s*input\.layerPriority/);
  assert.doesNotMatch(source, /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath/i);
});

test("install plan API invokes controlled install task command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");

  assert.match(source, /invoke<TaskStartedDto>\("start_install_task"/);
  assert.match(source, /start_install_task"[\s\S]*request:\s*\{/);
  assert.match(source, /gameId:\s*input\.gameId/);
  assert.match(source, /modId:\s*input\.modId/);
  assert.match(source, /profileId:\s*input\.profileId/);
  assert.match(source, /layerName:\s*input\.layerName/);
  assert.match(source, /layerPriority:\s*input\.layerPriority/);
  assert.doesNotMatch(source, /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath/i);
});

test("install plan API invokes controlled uninstall task command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");

  assert.match(source, /export function startUninstallTask/);
  const uninstallCall = source.match(/export function startUninstallTask[\s\S]*?\n}/);
  assert.ok(uninstallCall, "expected a feature-local uninstall wrapper");
  assert.match(uninstallCall[0], /invoke<TaskStartedDto>\("start_uninstall_task"/);
  assert.match(uninstallCall[0], /request:\s*\{/);
  assert.match(uninstallCall[0], /gameId:\s*input\.gameId/);
  assert.match(uninstallCall[0], /modId:\s*input\.modId/);
  assert.match(uninstallCall[0], /profileId:\s*input\.profileId/);
  assert.doesNotMatch(
    uninstallCall[0],
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|backupRoot|backupRef|layerName|layerPriority/i,
  );
});

test("install manifest status API invokes controlled summary command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");

  assert.match(source, /invoke<InstallManifestStatusSummary\[\]>\("get_install_manifest_status"/);
  assert.match(source, /get_install_manifest_status"[\s\S]*request:\s*\{/);
  assert.match(source, /profileId:\s*input\.profileId/);
  assert.match(source, /modIds:\s*input\.modIds/);
  assert.doesNotMatch(source, /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|backupRoot/i);
});

test("install recovery scan API invokes controlled summary command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");

  assert.match(source, /export function scanInstallRecovery/);
  const recoveryCall = source.match(/export function scanInstallRecovery[\s\S]*?\n}/);
  assert.ok(recoveryCall, "expected a feature-local recovery scan wrapper");
  assert.match(recoveryCall[0], /invoke<InstallRecoverySummary\[\]>\("scan_install_recovery"/);
  assert.match(recoveryCall[0], /request:\s*\{/);
  assert.match(recoveryCall[0], /gameId:\s*input\.gameId/);
  assert.match(recoveryCall[0], /profileId:\s*input\.profileId/);
  assert.match(recoveryCall[0], /modIds:\s*input\.modIds/);
  assert.doesNotMatch(
    recoveryCall[0],
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|backupRoot|backupRef/i,
  );
});

test("install recovery action preview API invokes controlled preview command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");
  const typesSource = readSource("src/features/mods/modInstallPlanTypes.ts");

  assert.match(source, /export function previewRecoveryAction/);
  const previewCall = source.match(/export function previewRecoveryAction[\s\S]*?\n}/);
  assert.ok(previewCall, "expected a feature-local recovery action preview wrapper");
  assert.match(previewCall[0], /invoke<InstallRecoveryActionPreview>\("preview_recovery_action"/);
  assert.match(previewCall[0], /request:\s*\{/);
  assert.match(previewCall[0], /gameId:\s*input\.gameId/);
  assert.match(previewCall[0], /profileId:\s*input\.profileId/);
  assert.match(previewCall[0], /modId:\s*input\.modId/);
  assert.match(previewCall[0], /actionKind:\s*input\.actionKind/);
  assert.doesNotMatch(
    previewCall[0],
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|backupRoot|backupRef|hash/i,
  );

  assert.match(typesSource, /export type InstallRecoveryActionKind\s*=\s*"rollback_install"/);
  assert.match(typesSource, /export type InstallRecoveryActionAvailability\s*=\s*"available"\s*\|\s*"blocked"/);
  assert.match(typesSource, /export type PreviewRecoveryActionInput/);
  assert.match(typesSource, /export type InstallRecoveryActionPreview/);
  assert.match(typesSource, /removeFileCount:\s*number/);
  assert.match(typesSource, /restoreFileCount:\s*number/);
  assert.match(typesSource, /blockingReasons:\s*InstallRecoveryActionBlockReasonSummary\[\]/);
  const actionPreviewType = typesSource.match(/export type InstallRecoveryActionPreview[\s\S]*?};/);
  assert.ok(actionPreviewType, "expected action preview DTO type");
  assert.doesNotMatch(actionPreviewType[0], /targetPath|manifestPath|backupRef|backupRoot|hash/i);
});

test("install recovery action task API invokes controlled task command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");
  const typesSource = readSource("src/features/mods/modInstallPlanTypes.ts");

  assert.match(source, /export function startRecoveryActionTask/);
  const taskCall = source.match(/export function startRecoveryActionTask[\s\S]*?\n}/);
  assert.ok(taskCall, "expected a feature-local recovery action task wrapper");
  assert.match(taskCall[0], /invoke<TaskStartedDto>\("start_recovery_action_task"/);
  assert.match(taskCall[0], /request:\s*\{/);
  assert.match(taskCall[0], /gameId:\s*input\.gameId/);
  assert.match(taskCall[0], /profileId:\s*input\.profileId/);
  assert.match(taskCall[0], /modId:\s*input\.modId/);
  assert.match(taskCall[0], /actionKind:\s*input\.actionKind/);
  assert.doesNotMatch(
    taskCall[0],
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|backupRoot|backupRef|hash/i,
  );

  assert.match(typesSource, /export type StartRecoveryActionTaskInput/);
  const inputType = typesSource.match(/export type StartRecoveryActionTaskInput[\s\S]*?};/);
  assert.ok(inputType, "expected recovery action task input type");
  assert.doesNotMatch(inputType[0], /targetPath|manifestPath|backupRef|backupRoot|hash/i);
});

test("install plan types expose preview DTO without filesystem paths", () => {
  assert.equal(existsSync("src/features/mods/modInstallPlanTypes.ts"), true);
  const source = readSource("src/features/mods/modInstallPlanTypes.ts");

  assert.match(source, /export type PreviewImportedModInstallPlanInput/);
  assert.match(source, /export type InstallPlanPreview/);
  assert.match(source, /hasBlockingConflicts:\s*boolean/);
  assert.match(source, /targetPath:\s*string/);
  assert.match(source, /packageFileId:\s*string/);
  assert.doesNotMatch(source, /sandbox|cache|localPath|diskPath|archivePath|allowedTargetRoots/i);
});

test("mod library page renders a backend install plan preview workflow", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /previewInstallPlanForImportedMod/);
  assert.match(source, /InstallPlanPreviewPanel/);
  assert.match(source, /preview-plan/);
  assert.match(source, /selectedIds\.size\s*!==\s*1/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath/i);

  const previewCall = source.match(/previewInstallPlanForImportedMod\(\{([\s\S]*?)\}\)/);
  assert.ok(previewCall, "expected page to call the backend-driven imported mod preview wrapper");
  assert.match(previewCall[1], /gameId:\s*DEFAULT_INSTALL_GAME_ID/);
  assert.match(previewCall[1], /modId/);
  assert.match(previewCall[1], /layerName:\s*"base"/);
  assert.match(previewCall[1], /layerPriority:\s*0/);
  assert.doesNotMatch(previewCall[1], /targetPath|allowedTargetRoots|sandbox|cache|archivePath/i);
});

test("mod library page starts install task and tracks only matching task progress", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");
  const taskStateSource = readSource("src/features/mods/modInstallTaskState.ts");

  assert.match(source, /startInstallTask/);
  assert.match(source, /TASK_PROGRESS_EVENT_NAME/);
  assert.match(source, /listen<\s*TaskProgressEventDto\s*>/);
  assert.match(source, /event\.payload\.taskId\s*!==\s*installTaskState\.taskId/);
  assert.match(source, /event\.payload\.kind\s*!==\s*"install"/);
  assert.match(source, /pendingInstallProgressEventsRef/);
  assert.match(source, /pendingInstallProgressEventsRef\.current\.set\(event\.payload\.taskId,\s*event\.payload\)/);
  assert.match(source, /pendingInstallProgressEventsRef\.current\.get\(task\.taskId\)/);
  assert.match(source, /nextManagedInstallTaskStateFromProgress\(runningState,\s*pendingProgressEvent\)/);
  assert.match(source, /canInstallSelected/);
  assert.match(source, /install\.queued/);
  assert.match(source, /install\.failed/);
  assert.match(taskStateSource, /install\.plan\.building/);
  assert.match(taskStateSource, /install\.commit\.processing/);
  assert.match(taskStateSource, /install\.completed/);
  assert.match(taskStateSource, /install\.cancelled/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot/i);
});

test("mod library page starts uninstall task only from manifest installed summaries", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");
  const taskStateSource = readSource("src/features/mods/modInstallTaskState.ts");

  assert.match(source, /startUninstallTask/);
  assert.match(source, /startSelectedUninstallTask/);
  assert.match(source, /selectedItem\?\.installSummary\?\.status\s*===\s*"installed"/);
  assert.match(source, /install\.uninstall\.queued/);
  assert.match(source, /install\.uninstall\.failed/);
  assert.match(source, /nextManagedInstallTaskStateFromProgress/);
  assert.match(taskStateSource, /install\.uninstall\.processing/);
  assert.match(taskStateSource, /install\.uninstall\.completed/);
  assert.match(source, /onConfirmUninstall/);
  assert.match(source, /refreshInstallManifestStatuses/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot|backupRef/i);
});

test("mod library page refreshes install status from manifest summaries", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /getInstallManifestStatus/);
  assert.match(source, /scanInstallRecovery/);
  assert.match(source, /applyInstallManifestStatusSummaries/);
  assert.match(source, /applyInstallRecoverySummaries/);
  assert.match(source, /applyInstallRecoveryUnavailable/);
  assert.match(source, /profileId:\s*DEFAULT_INSTALL_PROFILE_ID/);
  assert.match(source, /gameId:\s*DEFAULT_INSTALL_GAME_ID/);
  assert.match(source, /modIds/);
  assert.match(source, /installTaskState\.status\s*!==\s*"completed"/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot/i);
});

test("mod library page blocks install and uninstall actions during unsafe recovery states", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");
  const actionPanelSource = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(source, /canInstallSelected/);
  assert.match(source, /summary\?\.status\s*===\s*"rollback_required"/);
  assert.match(source, /summary\?\.status\s*===\s*"repair_required"/);
  assert.match(source, /summary\?\.status\s*===\s*"unknown"/);
  assert.match(source, /recoveryPanelStateForItem/);
  assert.match(source, /canInstallSelection=\{canInstallSelected\}/);
  assert.match(actionPanelSource, /canInstallSelection/);
  assert.match(actionPanelSource, /action\.id\s*===\s*"reinstall"\s*&&\s*!canInstallSelection/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot|backupRef/i);
});
