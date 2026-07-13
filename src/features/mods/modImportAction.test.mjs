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
  assert.doesNotMatch(source, /convertFileSrc|readFile|writeFile|removeFile|asset:|thumbnail:/);
  assert.doesNotMatch(source, /event\.payload\.(message|error)/);
});

test("the existing add action owns import and refreshes the library on completion", () => {
  const panelSource = readSource("src/features/mods/CompactActionPanel.tsx");
  const pageSource = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(panelSource, /<ModImportAction\s+label=\{addAction\.label\}\s+onImported=\{onImportCompleted\}/);
  assert.match(pageSource, /onImportCompleted=\{refreshModLibrary\}/);
});
