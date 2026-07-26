import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("task progress hook owns listener, launch buffering, notices, and cancel re-entry", () => {
  const source = readSource(
    "src/features/mods/external-import/useExternalImportTaskProgress.ts",
  );

  assert.match(source, /listen<TaskProgressEventDto>\(TASK_PROGRESS_EVENT_NAME/);
  assert.match(source, /event\.payload\.kind\s*!==\s*"mod_import"/);
  assert.match(source, /event\.payload\.taskId\s*!==\s*taskId/);
  assert.match(source, /startPendingRef\.current/);
  assert.match(source, /pendingProgressEventsRef\.current\.set\(event\.payload\.taskId/);
  assert.match(source, /pendingProgressEventsRef\.current\.get\(launch\.task\.taskId\)/);
  assert.match(source, /nextExternalImportTaskStateFromProgress/);
  assert.match(source, /isExternalImportBatchStartedDto\(launch,\s*expectedBatchId\)/);
  assert.match(source, /cancelPendingRef\.current/);
  assert.match(source, /const launchImport = useCallback/);
  assert.match(source, /const cancelImport = useCallback/);
  assert.match(source, /const retryListener = useCallback/);
  assert.match(source, /showTaskNotice\(\{/);
  assert.match(source, /dismissTaskNotice\(previousTaskId\)/);
  assert.doesNotMatch(source, /event\.payload\.(message|error)/);
});

test("selection workflow delegates task concerns without changing selection ownership", () => {
  const source = readSource(
    "src/features/mods/external-import/useExternalImportSelectionWorkflow.ts",
  );

  assert.match(source, /useExternalImportTaskProgress\(batchId\)/);
  assert.match(
    source,
    /const\s*\{[\s\S]{0,240}launchImport,[\s\S]{0,160}cancelImport,[\s\S]{0,80}\}\s*=\s*useExternalImportTaskProgress\(batchId\)/,
  );
  assert.match(source, /isImportActive,\s*launchImport,\s*listenerStatus,\s*\]\);/);
  assert.doesNotMatch(source, /taskProgress\./);
  assert.match(source, /startExternalImportBatch/);
  assert.match(source, /setTrackedSelection\(\{\s*\.\.\.currentSelection,\s*status:\s*"sealed"/);
  assert.doesNotMatch(source, /listen<TaskProgressEventDto>/);
  assert.doesNotMatch(source, /pendingProgressEventsRef|displayedTaskNoticeIdRef|cancelPendingRef/);
});
