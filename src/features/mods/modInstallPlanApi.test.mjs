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
  const typesSource = readSource("src/features/mods/modInstallPlanTypes.ts");

  const manifestStatusCall = source.match(/export function getInstallManifestStatus[\s\S]*?\n}/);
  assert.ok(manifestStatusCall, "expected a feature-local manifest status wrapper");
  assert.match(manifestStatusCall[0], /invoke<InstallManifestStatusSummary\[\]>\("get_install_manifest_status"/);
  assert.match(manifestStatusCall[0], /request:\s*\{/);
  assert.match(manifestStatusCall[0], /input\.gameId\s*===\s*undefined\s*\?\s*\{\}\s*:\s*\{\s*gameId:\s*input\.gameId\s*\}/);
  assert.match(manifestStatusCall[0], /profileId:\s*input\.profileId/);
  assert.match(manifestStatusCall[0], /modIds:\s*input\.modIds/);
  assert.match(typesSource, /export type GetInstallManifestStatusInput = \{\s*gameId\?:\s*GameId;/);
  const manifestStatusType = typesSource.match(/export type InstallManifestStatus[\s\S]*?;/);
  assert.ok(manifestStatusType, "expected InstallManifestStatus union");
  assert.match(manifestStatusType[0], /"committed_cleanup_pending"/);
  assert.match(manifestStatusType[0], /"cleanup_pending"/);
  assert.match(manifestStatusType[0], /"rollback_required"/);
  assert.doesNotMatch(
    manifestStatusCall[0],
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|backupRoot/i,
  );
});

test("install recovery scan API invokes controlled summary command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");
  const typesSource = readSource("src/features/mods/modInstallPlanTypes.ts");

  assert.match(source, /export function scanInstallRecovery/);
  const recoveryCall = source.match(/export function scanInstallRecovery[\s\S]*?\n}/);
  assert.ok(recoveryCall, "expected a feature-local recovery scan wrapper");
  assert.match(recoveryCall[0], /invoke<InstallRecoverySummary\[\]>\("scan_install_recovery"/);
  assert.match(recoveryCall[0], /request:\s*\{/);
  assert.match(recoveryCall[0], /gameId:\s*input\.gameId/);
  assert.match(recoveryCall[0], /profileId:\s*input\.profileId/);
  assert.match(recoveryCall[0], /modIds:\s*input\.modIds/);
  const recoveryStatusType = typesSource.match(/export type InstallRecoveryStatus[\s\S]*?;/);
  assert.ok(recoveryStatusType, "expected InstallRecoveryStatus union");
  assert.match(recoveryStatusType[0], /"committed_cleanup_pending"/);
  assert.match(recoveryStatusType[0], /"cleanup_pending"/);
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

  const recoveryActionKind = typesSource.match(/export type InstallRecoveryActionKind[\s\S]*?;/);
  assert.ok(recoveryActionKind, "expected InstallRecoveryActionKind union");
  assert.match(recoveryActionKind[0], /"rollback_install"/);
  assert.match(recoveryActionKind[0], /"reconcile_reinstall"/);
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
  assert.match(source, /prerequisiteDecision:\s*GamePrerequisiteDecision/);
  assert.match(source, /status:\s*GamePrerequisiteDecisionStatus/);
  assert.match(source, /rulesVersion:\s*number\s*\|\s*null/);
  assert.match(source, /"missing_required_file"/);
  assert.match(source, /"signature_unverified"/);
  assert.match(source, /targetPath:\s*string/);
  assert.match(source, /packageFileId:\s*string/);
  assert.doesNotMatch(source, /sandbox|cache|localPath|diskPath|archivePath|allowedTargetRoots/i);
});

test("install plan sheet renders backend prerequisite decision without rebuilding rules", () => {
  const panel = readSource("src/features/mods/ModLifecycleFeedback.tsx");
  const labels = readSource("src/features/mods/modLifecycleCopy.ts");

  assert.match(panel, /plan\.prerequisiteDecision/);
  assert.match(panel, /getPrerequisiteDecisionMessage/);
  assert.match(panel, /getPrerequisiteDecisionCodeLabel/);
  assert.match(labels, /missing_required_file:\s*"缺少必要前置文件"/);
  assert.match(labels, /signature_unverified:\s*"前置文件签名无法验证"/);
  assert.doesNotMatch(panel, /dinput8|loader-config|nativePC|issue\.path/);
});

test("mod library page renders a backend install plan preview workflow", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");
  const feedbackSource = readSource("src/features/mods/ModLifecycleFeedback.tsx");

  assert.match(source, /previewInstallPlanForImportedMod/);
  assert.match(source, /InstallPlanDetailSheet/);
  assert.match(feedbackSource, /DetailSheet/);
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
  assert.match(
    source,
    /nextManagedInstallTaskStateFromProgress\(\s*runningState,\s*pendingProgressEvent,/,
  );
  assert.match(source, /canInstallSelected/);
  assert.match(source, /install\.queued/);
  assert.match(source, /install\.failed/);
  assert.match(taskStateSource, /install\.plan\.building/);
  assert.match(taskStateSource, /install\.commit\.processing/);
  assert.match(taskStateSource, /install\.completed/);
  assert.match(taskStateSource, /install\.cancelled/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot|backupRef/i);
});

test("mod library page starts uninstall only from a durable installed summary on the current page", () => {
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
  assert.match(source, /UninstallConfirmationDialog/);
  assert.match(source, /onConfirm=\{startSelectedUninstallTask\}/);
  assert.match(source, /refreshModLibraryDurableStatuses\(page\.items/);
  assert.match(source, /loadManifestStatuses:[\s\S]*?getInstallManifestStatus/);
  assert.match(source, /loadRecoveryStatuses:[\s\S]*?scanInstallRecovery/);
  assert.match(source, /const currentItem = libraryItems\.find\(\(item\) => item\.id === uninstallConfirmation\.modId\)/);
  assert.match(source, /currentSummary\?\.status\s*!==\s*"installed"/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot|backupRef/i);
});

test("mod library page overlays the current query page and verifies terminal facts independently", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");
  const refreshSource = readSource("src/features/mods/modLibraryRecoveryRefresh.ts");

  assert.match(source, /queryModLibrary/);
  assert.match(source, /refreshModLibraryDurableStatuses\(page\.items/);
  assert.match(source, /getInstallManifestStatus/);
  assert.match(source, /scanInstallRecovery/);
  assert.match(source, /useActiveProfile/);
  assert.match(source, /input\.profileContext\s*===\s*undefined/);
  assert.match(source, /profileId:\s*activeProfileId/);
  assert.match(source, /gameId:\s*DEFAULT_INSTALL_GAME_ID/);
  assert.match(source, /modIds/);
  assert.match(refreshSource, /items\.map\(\(item\) => item\.id\)/);
  assert.match(refreshSource, /applyInstallManifestStatusSummaries\(items,\s*manifestStatuses\)/);
  assert.match(refreshSource, /applyInstallRecoverySummaries\(itemsWithManifestStatus,\s*recoveryStatuses\)/);
  assert.match(refreshSource, /items:\s*applyInstallManifestUnavailable\(items\)/);
  assert.match(refreshSource, /items:\s*applyInstallRecoveryUnavailable\(itemsWithManifestStatus\)/);
  assert.match(source, /isManagedInstallTaskTerminal\(installTaskState\)/);
  assert.match(source, /refreshTerminalDurableStatus/);
  assert.match(source, /createModLibraryStatusProbe\(modId,\s*modName\)/);
  assert.match(source, /Promise\.allSettled\(\[/);
  assert.match(source, /getManagedInstallTerminalToast/);
  assert.match(source, /shouldFailClosedManagedInstallTerminal/);
  assert.match(source, /updateCurrentPageItems\(\(items\)\s*=>\s*failClosedModInstallSummary\(items,\s*terminalTask\.modId\)\)/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot|backupRef/i);
});

test("mod library page blocks install and uninstall actions during unsafe recovery states", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");
  const actionPanelSource = readSource("src/features/mods/CompactActionPanel.tsx");
  const availabilitySource = readSource("src/features/mods/compactActionAvailability.ts");
  const feedbackSource = readSource("src/features/mods/ModLifecycleFeedback.tsx");

  assert.match(source, /canInstallSelected/);
  assert.match(source, /isUnsafeInstallStatus\(summary\?\.status\s*\?\?\s*""\)/);
  assert.match(source, /selectedItem\.installSummary\?\.status\s*===\s*"not_installed"/);
  assert.match(source, /recoveryPanelStateForItem/);
  assert.match(source, /canInstallSelection=\{canInstallSelected\}/);
  assert.match(source, /canReinstallSelection=\{canReinstallSelected\}/);
  assert.match(actionPanelSource, /canInstallSelection/);
  assert.match(actionPanelSource, /getCompactActionDisabledReason/);
  assert.match(availabilitySource, /case "install":[\s\S]*?canInstallSelection \? undefined/);
  assert.match(availabilitySource, /case "reinstall":[\s\S]*?canReinstallSelection \? undefined/);
  assert.match(availabilitySource, /case "uninstall":[\s\S]*?canUninstallSelection \? undefined/);
  const lifecycleCopySource = readSource("src/features/mods/modLifecycleCopy.ts");
  assert.match(lifecycleCopySource, /committed_cleanup_pending: "重装待收尾"/);
  assert.match(lifecycleCopySource, /cleanup_pending: "恢复待清理"/);
  assert.match(feedbackSource, /planSheet\.recoveryTitles\[status\]/);
  assert.match(feedbackSource, /planSheet\.recoveryMessages\[status\]/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot|backupRef/i);
});
