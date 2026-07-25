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
  assert.match(source, /isExternalImportOpaqueId\(value\.task\.taskId\)/);
  assert.match(source, /isExternalImportOpaqueId\(value\.batchId\)/);
  assert.match(source, /showTaskNotice\(\{/);
  assert.match(source, /dismissTaskNotice\(previousTaskId\)/);
  assert.match(source, /listenerStatus === "failed"/);
  assert.doesNotMatch(source, /external_import\.listener\.failed/);
  assert.doesNotMatch(source, /id=\{statusId\}|aria-describedby=\{statusId\}/);
  assert.doesNotMatch(source, /\[\s*previewState\s*,\s*setPreviewState\s*\]/);
  assert.doesNotMatch(source, /event\.payload\.(message|error)/);
});

test("external import action composes the 4B selection panel without a frontend file picker", () => {
  const source = readSource("src/features/mods/external-import/ExternalImportAction.tsx");
  const selectionPanel = readSource(
    "src/features/mods/external-import/ExternalImportSelectionPanel.tsx",
  );
  const selectionWorkflow = readSource(
    "src/features/mods/external-import/useExternalImportSelectionWorkflow.ts",
  );
  const candidateSelection = readSource(
    "src/features/mods/external-import/ExternalImportCandidateSelectionItem.tsx",
  );
  const panelSource = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(source, /<Dialog/);
  assert.match(source, /selectExternalImportSource/);
  assert.match(source, /startExternalImportScan/);
  assert.match(source, /cancelExternalImportScan/);
  assert.match(source, /useExternalImportSelectionWorkflow\(/);
  assert.match(source, /<ExternalImportSelectionPanel/);
  assert.match(source, /scanState\.status\s*===\s*"completed"\s*\?\s*batchId\s*:\s*null/);
  assert.match(selectionWorkflow, /createExternalImportSelection/);
  assert.match(selectionWorkflow, /updateExternalImportSelection/);
  assert.match(selectionWorkflow, /selectAllExternalImportCandidates/);
  assert.match(selectionWorkflow, /startExternalImportBatch/);
  assert.match(selectionWorkflow, /listCategories/);
  assert.match(selectionWorkflow, /listen<TaskProgressEventDto>\(TASK_PROGRESS_EVENT_NAME/);
  assert.match(selectionWorkflow, /event\.payload\.taskId\s*!==\s*taskId/);
  assert.match(selectionWorkflow, /nextExternalImportTaskStateFromProgress/);
  assert.match(selectionWorkflow, /currentPreview\.loadingMore/);
  assert.match(selectionWorkflow, /workflowGenerationRef\.current === expectedGeneration/);
  assert.match(selectionWorkflow, /isExternalImportSelectionExpired\(current, Date\.now\(\)\)/);
  assert.match(selectionWorkflow, /window\.setTimeout\(markExpired, delay\)/);
  assert.match(source, /selectionWorkflow\.pendingAction !== null/);
  assert.match(source, /selectionWorkflow\.previewState\.status === "loading"/);
  assert.match(source, /selectionWorkflow\.previewState\.loadingMore/);
  assert.match(
    selectionWorkflow,
    /currentSelection === null[\s\S]*initializeSelection\(currentBatchId/,
  );
  assert.match(selectionPanel, /workflow\.loadMore/);
  assert.doesNotMatch(selectionPanel, /listen<TaskProgressEventDto>/);
  assert.match(candidateSelection, /getRequiredExternalImportConflictResolution/);
  assert.match(candidateSelection, /value="keep_both"|requiredResolution/);
  assert.match(candidateSelection, /ignore_invalid_metadata|resolutionLabel/);
  assert.doesNotMatch(candidateSelection, /sourceModType/);
  assert.match(source, /isExternalImportSourceDto\(selectedSource\)/);
  assert.match(panelSource, /<ExternalImportAction\s*\/>/);
  assert.doesNotMatch(source, /@tauri-apps\/plugin-dialog|open\(\{/);
  assert.doesNotMatch(
    `${selectionPanel}\n${candidateSelection}\n${selectionWorkflow}`,
    /retryExternalImportBatch|getExternalImportBatchResult/,
  );
  assert.doesNotMatch(
    `${source}\n${selectionPanel}\n${candidateSelection}\n${selectionWorkflow}`,
    /readFile|writeFile|removeFile|convertFileSrc|asset:|thumbnail:|sandbox|cache|archivePath/i,
  );
});
