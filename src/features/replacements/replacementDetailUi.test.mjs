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
  assert.match(panel, /previewState\.preview\.prerequisiteDecision\.status/);
  assert.match(panel, /getPrerequisiteDecisionMessage/);
  assert.match(panel, /getPrerequisiteDecisionCodeLabel/);
  assert.match(panel, /TASK_PROGRESS_EVENT_NAME/);
  assert.match(panel, /event\.payload\.taskId/);
  assert.match(panel, /refreshRetargetInstallState/);
  assert.match(panel, /completionReloadPendingRef\.current = true;[\s\S]*setRetryToken/);
  assert.match(
    panel,
    /completionReloadPendingRef\.current = false;[\s\S]*setRefreshState\(\{ status: "ready" \}\);[\s\S]*setTrackedTaskState\(\{ status: "idle" \}\)/,
  );
  assert.match(panel, /重试刷新/);
  assert.match(panel, /weapon_partial_part_set/);
  assert.match(panel, /target\.catalogScope === "developer_sandbox"/);
  assert.doesNotMatch(panel, /source\.pathFamily/);
  assert.doesNotMatch(panel, /action\.sourceRelativePath|action\.targetRelativePath/);
  assert.doesNotMatch(
    panel,
    /packageId:|sourceId:|bindingId:|sandboxPath|stagingPath|gameRoot|archivePath/i,
  );
});

test("MOD file edit context action opens the existing detail panel on replacement tab", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const refresh = readSource("src/features/mods/modLibraryRefresh.ts");

  assert.match(page, /case "edit-files"[\s\S]*createDetailDialogState\([^)]*"replacement"/);
  assert.match(page, /initialTab=\{detailDialogState\.initialTab\}/);
  assert.match(refresh, /initialTab:\s*ModDetailDialogTab/);
  assert.doesNotMatch(page, /MOD 文件修改功能开发中/);
});

test("replacement panel blocks stay legible on the dialog's tinted content area", () => {
  const panelCss = readSource("src/features/replacements/ReplacementTargetPanel.css");
  const dialogCss = readSource("src/features/mods/ModDetailDialog.css");

  /*
   * 该面板嵌在详情对话框的内容区里，而内容区背景是 --color-surface-subtle。
   * 面板内部的可读区块（搜索框、统计卡、来源事实、动作列表、空态、任务状态条）
   * 曾经也用同一个 surface-subtle 做底色，于是在对话框改为浅底内容区后整片隐形。
   * 这条断言锁住"浅底托白块"的方向：内容区是浅底，面板内的块必须是白底。
   */
  const bodyRule = dialogCss.match(/\.mod-detail-dialog__body\s*\{([\s\S]*?)\}/)?.[1] ?? "";
  assert.match(
    bodyRule,
    /background:\s*var\(--color-surface-subtle\)/,
    "对话框内容区不再是浅底，本断言的前提已变，需要重新评估面板底色",
  );
  /*
   * 只检查区块的静态底色。:hover / :focus 等交互态用 surface-subtle 是有意的——
   * 它们叠在白底块之上作为反馈，不会与内容区背景混淆。
   */
  const staticSubtleBlocks = [...panelCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter(
    ([, selector, body]) =>
      /background:\s*var\(--color-surface-subtle\)/.test(body)
      && !/:hover|:focus|:active/.test(selector),
  );
  assert.deepEqual(
    staticSubtleBlocks.map(([, selector]) => selector.trim()),
    [],
    "面板区块使用了与内容区相同的浅底，会与背景融为一体",
  );

  // 等宽字体走 token，不再是悬空引用（浏览器回落 monospace，在 Windows 上是 Courier New）。
  assert.match(panelCss, /font-family:\s*var\(--font-family-mono\)/);
  assert.doesNotMatch(panelCss, /var\(--font-family-mono,\s*monospace\)/);
  const tokens = readSource("src/shared/styles/tokens.css");
  assert.equal(
    tokens.match(/--font-family-mono:/g)?.length,
    2,
    "--font-family-mono 必须在浅色与深色两套 token 中都定义",
  );

  // hover 与选中不得同色，否则扫一眼分不出当前选的是哪一项。
  assert.match(
    panelCss,
    /\.replacement-panel__target-row:hover:not\(\[data-selected="true"\]\)/,
  );
});
