import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("external import action owns a task-scoped listener and keeps early events buffered", () => {
  const source = readSource("src/features/mods/external-import/ExternalImportAction.tsx");

  assert.match(source, /listen<TaskProgressEventDto>\(TASK_PROGRESS_EVENT_NAME/);
  assert.match(source, /event\.payload\.kind\s*!==\s*"mod_import"/);
  assert.match(source, /event\.payload\.taskId\s*!==\s*taskId/);
  assert.match(source, /startPendingRef\.current/);
  assert.match(source, /pendingProgressEventsRef\.current\.set\(event\.payload\.taskId/);
  assert.match(source, /pendingProgressEventsRef\.current\.get\(launch\.task\.taskId\)/);
  assert.match(source, /nextExternalImportScanTaskStateFromProgress/);
  assert.match(source, /showTaskNotice\(\{/);
  assert.match(source, /dismissTaskNotice\(previousTaskId\)/);
  assert.match(source, /listenerStatus === "failed"/);
  assert.doesNotMatch(source, /external_import\.listener\.failed/);
  assert.doesNotMatch(source, /id=\{statusId\}|aria-describedby=\{statusId\}/);
  assert.match(source, /scanState\.status === "completed"\s*&&\s*previewState\.status === "idle"/);
  assert.doesNotMatch(source, /event\.payload\.(message|error)/);
});

test("external import action is a read-only preview surface without a frontend file picker", () => {
  const source = readSource("src/features/mods/external-import/ExternalImportAction.tsx");
  const panelSource = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(source, /<Dialog/);
  assert.match(source, /selectExternalImportSource/);
  assert.match(source, /startExternalImportScan/);
  assert.match(source, /getExternalImportPreview/);
  assert.match(source, /cancelExternalImportScan/);
  assert.match(source, /isExternalImportSourceDto\(selectedSource\)/);
  assert.match(source, /loadMore/);
  assert.match(panelSource, /<ExternalImportAction\s*\/>/);
  assert.doesNotMatch(source, /@tauri-apps\/plugin-dialog|open\(\{/);
  assert.doesNotMatch(source, /create_external_import_selection|update_external_import_selection|select_all_external_import_candidates/);
  assert.doesNotMatch(source, /start_external_import_batch|retry_external_import_batch|get_external_import_batch_result/);
  assert.doesNotMatch(source, /readFile|writeFile|removeFile|convertFileSrc|asset:|thumbnail:|sandbox|cache|archivePath/i);
});
