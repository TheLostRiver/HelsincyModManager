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
  assert.match(source, /isImportTaskTerminal\(next\)[\s\S]*taskIdRef\.current\s*=\s*null/);
  assert.match(source, /showTaskNotice\(\{/);
  assert.match(source, /taskId:\s*taskState\.taskId/);
  assert.match(source, /dismissTaskNotice\(previousTaskId\)/);
  assert.doesNotMatch(source, /convertFileSrc|readFile|writeFile|removeFile|asset:|thumbnail:/);
  assert.doesNotMatch(source, /event\.payload\.(message|error)/);
});

test("the existing add action owns import and refreshes the library on completion", () => {
  const panelSource = readSource("src/features/mods/CompactActionPanel.tsx");
  const pageSource = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(panelSource, /<ModImportAction\s+label=\{addAction\.label\}\s+onImported=\{onImportCompleted\}/);
  assert.match(pageSource, /onImportCompleted=\{refreshModLibrary\}/);
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
