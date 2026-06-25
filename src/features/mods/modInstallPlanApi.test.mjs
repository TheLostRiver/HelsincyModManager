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
  assert.match(source, /gameId:\s*input\.gameId/);
  assert.match(source, /modId:\s*input\.modId/);
  assert.match(source, /layerName:\s*input\.layerName/);
  assert.match(source, /layerPriority:\s*input\.layerPriority/);
  assert.doesNotMatch(source, /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath/i);
});

test("install plan API invokes controlled install task command without paths", () => {
  const source = readSource("src/features/mods/modInstallPlanApi.ts");

  assert.match(source, /invoke<TaskStartedDto>\("start_install_task"/);
  assert.match(source, /gameId:\s*input\.gameId/);
  assert.match(source, /modId:\s*input\.modId/);
  assert.match(source, /profileId:\s*input\.profileId/);
  assert.match(source, /layerName:\s*input\.layerName/);
  assert.match(source, /layerPriority:\s*input\.layerPriority/);
  assert.doesNotMatch(source, /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath/i);
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
  assert.match(previewCall[1], /gameId:\s*"mhw"/);
  assert.match(previewCall[1], /modId/);
  assert.match(previewCall[1], /layerName:\s*"base"/);
  assert.match(previewCall[1], /layerPriority:\s*0/);
  assert.doesNotMatch(previewCall[1], /targetPath|allowedTargetRoots|sandbox|cache|archivePath/i);
});

test("mod library page starts install task and tracks only matching task progress", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /startInstallTask/);
  assert.match(source, /TASK_PROGRESS_EVENT_NAME/);
  assert.match(source, /listen<\s*TaskProgressEventDto\s*>/);
  assert.match(source, /event\.payload\.taskId\s*!==\s*installTaskState\.taskId/);
  assert.match(source, /event\.payload\.kind\s*!==\s*"install"/);
  assert.match(source, /install\.queued/);
  assert.match(source, /install\.plan\.building/);
  assert.match(source, /install\.commit\.processing/);
  assert.match(source, /install\.completed/);
  assert.match(source, /install\.failed/);
  assert.match(source, /install\.cancelled/);
  assert.doesNotMatch(source, /targetPath:\s*|allowedTargetRoots|archivePath|manifestPath|backupRoot/i);
});
