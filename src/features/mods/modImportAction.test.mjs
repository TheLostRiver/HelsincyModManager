import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("mod import action opens a ZIP picker and starts the controlled task", () => {
  const source = readSource("src/features/mods/ModImportAction.tsx");

  assert.match(source, /open\(\{/);
  assert.match(source, /extensions:\s*\["zip"\]/);
  assert.match(source, /startImportModTask\(\{\s*archivePath:\s*selected\s*\}\)/);
  assert.match(source, /event\.payload\.kind\s*!==\s*"mod_import"/);
  assert.match(source, /event\.payload\.taskId\s*!==\s*taskId/);
  assert.match(source, /setListenerAttempt\(\(attempt\)\s*=>\s*attempt\s*\+\s*1\)/);
  assert.match(source, /listenerStatus\s*===\s*"failed"[\s\S]*retryTaskProgressListener\(\)/);
  assert.match(source, /continueImportAfterReconnectRef\.current = true/);
  assert.match(source, /listenerStatus !== "ready" \|\| !continueImportAfterReconnectRef\.current/);
  assert.match(source, /void handleImportRef\.current\(\)/);
  assert.match(source, /mode === "revision" \? "导入新版本" : "导入 Mod"/);
  assert.match(source, /导入服务暂时不可用，点击后将自动重连并继续/);
  assert.match(source, /isImportTaskTerminal\(next\)[\s\S]*taskIdRef\.current\s*=\s*null/);
  assert.match(source, /showTaskNotice\(\{/);
  assert.match(source, /taskId:\s*taskState\.taskId/);
  assert.match(source, /dismissTaskNotice\(previousTaskId\)/);
  assert.match(source, /Promise\.resolve\(\)[\s\S]*?\.then\(\(\) => onImportedRef\.current\(\)\)[\s\S]*?mod-import\.refresh-failed/);
  assert.doesNotMatch(source, /convertFileSrc|readFile|writeFile|removeFile|asset:|thumbnail:/);
  assert.doesNotMatch(source, /event\.payload\.(message|error)/);
});

test("the existing add action owns import and refreshes the library on completion", () => {
  const panelSource = readSource("src/features/mods/CompactActionPanel.tsx");
  const pageSource = readSource("src/features/mods/ModLibraryPage.tsx");
  const dataSource = readSource("src/features/mods/modsLibraryData.ts");

  assert.match(panelSource, /<ModImportAction\s+label=\{addAction\.label\}\s+onImported=\{onImportCompleted\}/);
  assert.match(pageSource, /onImportCompleted=\{refreshModLibraryAfterWrite\}/);
  assert.match(dataSource, /id: "add", label: "导入 Mod"/);
});

test("candidate import reuses the picker task UI with an explicit selected mod owner", () => {
  const actionSource = readSource("src/features/mods/ModImportAction.tsx");
  const panelSource = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(actionSource, /mode\??:\s*"new"\s*\|\s*"revision"/);
  assert.match(actionSource, /startImportModRevisionTask\(\{\s*archivePath:\s*selected,\s*modId/);
  assert.match(actionSource, /useId\(\)/);
  assert.doesNotMatch(actionSource, /(?:displayName|author|versionLabel)[\s\S]*startImportModRevisionTask/);
  assert.match(panelSource, /mode="revision"/);
  assert.match(panelSource, /modId=\{selectedModId\}/);
  assert.match(panelSource, /disabledReason=/);
});
