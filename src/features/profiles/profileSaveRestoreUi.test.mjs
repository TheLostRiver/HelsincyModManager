import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(relativePath) {
  return readFileSync(new URL(`../../../${relativePath}`, import.meta.url), "utf8");
}

test("save restore dialog waits for its listener and keeps terminal state visible", () => {
  const dialog = readSource("src/features/profiles/SaveRestoreDialog.tsx");
  const page = readSource("src/features/profiles/ProfilePage.tsx");

  assert.match(dialog, /listenerStatus === "ready"/);
  assert.match(dialog, /ProfileSaveRestoreEarlyEventBuffer/);
  assert.match(dialog, /pendingEventsRef\.current\.take\(task\.taskId\)/);
  assert.match(dialog, /taskIdRef\.current !== event\.taskId/);
  assert.match(dialog, /setTaskState\(\(current\) => nextProfileSaveRestoreTaskStateFromProgress/);
  assert.match(dialog, /onCompletedRef\.current\(\)/);
  assert.doesNotMatch(dialog, /setState\(\(current\)[\s\S]*?pushToast/);
  assert.doesNotMatch(page, /onCompleted=\{\(\) => \{[\s\S]*?setRestoreBackup\(null\)/);
  assert.match(dialog, /taskState\.status === "recovery_required"/);
  assert.match(dialog, /请保留当前现场并联系支持/);
  assert.doesNotMatch(dialog, /恢复中心处理/);
  assert.match(dialog, /cancelProfileSaveRestoreTask/);
  assert.match(dialog, /taskState\.warningCodes\.map/);
});

test("save restore dialog uses backup game identity and preview remains closable", () => {
  const dialog = readSource("src/features/profiles/SaveRestoreDialog.tsx");

  assert.match(dialog, /gameId:\s*backup\.gameId/);
  assert.match(dialog, /gameId:\s*selectedBackup\.gameId/);
  assert.doesNotMatch(dialog, /gameId:\s*"mhw"/);
  assert.match(dialog, /const taskBusy = taskState\.status === "starting"/);
  assert.doesNotMatch(dialog, /previewState\.status === "previewing"[\s\S]{0,120}taskBusy/);
});

test("backup history exposes live restore and labels pre-restore protection points", () => {
  const page = readSource("src/features/profiles/ProfilePage.tsx");

  assert.match(page, /aria-label=\{`恢复存档：\$\{row\.name\}`\}/);
  assert.doesNotMatch(page, /功能即将开放/);
  assert.match(page, /trigger === "pre_restore"\) return "恢复前安全备份"/);
  assert.match(page, /恢复暂不可用：\{restoreBlockedReason\}/);
  assert.match(page, /if \(dirty\) return "请先保存存档设置"/);
  assert.match(page, /disabled=\{row\.backup\.status !== "completed" \|\| restoreBlockedReason !== null\}/);
});
