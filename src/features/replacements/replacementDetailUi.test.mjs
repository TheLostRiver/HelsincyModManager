import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("Mod detail unified panel owns the replacement target tab", () => {
  assert.equal(existsSync("src/features/replacements/ReplacementTargetPanel.tsx"), true);
  const dialog = readSource("src/features/mods/ModDetailDialog.tsx");
  const dialogCss = readSource("src/features/mods/ModDetailDialog.css");
  const panel = readSource("src/features/replacements/ReplacementTargetPanel.tsx");

  assert.match(dialog, /type ModDetailDialogTab = "details" \| "replacement"/);
  assert.match(dialog, /role="tablist"/);
  assert.match(dialog, /createPortal\([\s\S]*document\.body/);
  assert.match(dialog, /mod-detail-dialog__body[^\n]*is-replacement/);
  assert.match(
    dialogCss,
    /@media \(max-width: 760px\)[\s\S]*\.mod-detail-dialog__body\.is-replacement[\s\S]*order:\s*-1/,
  );
  assert.match(dialog, /替换目标/);
  assert.match(dialog, /<ReplacementTargetPanel/);
  assert.match(dialog, /replacementCompletedLocally/);
  assert.match(dialog, /completedLocally=\{replacementCompletedLocally\}/);
  assert.match(dialog, /installStatus=\{replacementInstallStatus\}/);
  assert.match(
    dialog,
    /await onSaved\(\);[\s\S]*setReplacementInstallStatus\("installed"\);[\s\S]*setReplacementCompletedLocally\(false\)/,
  );
  const tabs = dialog.match(/<div className="mod-detail-dialog__tabs"[\s\S]*?<\/div>/);
  assert.ok(tabs, "expected details and replacement tabs");
  assert.equal(tabs[0].match(/disabled=\{dialogBusy\}/g)?.length, 2);
  assert.match(panel, /listReplacementTargets/);
  assert.match(panel, /analyzeImportedModReplacement/);
  assert.match(panel, /previewInitialRetargetInstall/);
  assert.match(panel, /startRetargetInstallTask/);
  assert.match(panel, /previewRetargetReinstall/);
  assert.match(panel, /startRetargetReinstallTask/);
  assert.match(panel, /cancelRetargetInstallTask/);
  assert.match(panel, /取消任务/);
  assert.match(panel, /task_cannot_be_cancelled/);
  assert.match(panel, /install\.reinstall/);
  assert.match(panel, /当前目标已安装/);
  assert.match(panel, /data-installed=\{currentInstalled\}/);
  assert.match(panel, /analysis\.installedTargetId/);
  assert.match(panel, /保留/);
  assert.match(panel, /替换/);
  assert.match(panel, /新增/);
  assert.match(panel, /移除旧项/);
  assert.match(panel, /TASK_PROGRESS_EVENT_NAME/);
  assert.match(panel, /event\.payload\.taskId/);
  assert.match(panel, /refreshRetargetInstallState/);
  assert.match(panel, /completionReloadPendingRef\.current = true;[\s\S]*setRetryToken/);
  assert.match(
    panel,
    /completionReloadPendingRef\.current = false;[\s\S]*setRefreshState\(\{ status: "ready" \}\);[\s\S]*setTrackedTaskState\(\{ status: "idle" \}\)/,
  );
  assert.match(panel, /重试刷新/);
  assert.doesNotMatch(panel, /packageId:|sourceId:|bindingId:|sandbox|staging|gameRoot|archivePath/i);
});

test("MOD file edit context action opens the existing detail panel on replacement tab", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const refresh = readSource("src/features/mods/modLibraryRefresh.ts");

  assert.match(page, /case "edit-files"[\s\S]*createDetailDialogState\([^)]*"replacement"/);
  assert.match(page, /initialTab=\{detailDialogState\.initialTab\}/);
  assert.match(refresh, /initialTab:\s*ModDetailDialogTab/);
  assert.doesNotMatch(page, /MOD 文件修改功能开发中/);
});
