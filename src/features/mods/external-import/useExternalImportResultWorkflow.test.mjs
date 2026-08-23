import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("result workflow queries terminal tasks with request and batch identity gates", () => {
  const source = readSource(
    "src/features/mods/external-import/useExternalImportResultWorkflow.ts",
  );

  assert.match(source, /getExternalImportBatchResult\(\{/);
  assert.match(source, /cursor:\s*null/);
  assert.match(source, /isExternalImportBatchResultPageForBatch\(/);
  assert.match(source, /resultRequestRef\.current\s*!==\s*requestId/);
  assert.match(source, /batchIdRef\.current\s*!==\s*expectedBatchId/);
  assert.match(source, /terminalTaskIdRef\.current\s*!==\s*expectedTaskId/);
  assert.match(source, /const batchChanged = observedBatchId !== batchId/);
  assert.match(source, /if \(batchChanged\) \{[\s\S]{0,160}resultRequestRef\.current \+= 1/);
  assert.match(source, /state:\s*visibleState/);
  assert.match(
    source,
    /page\.results\.map\(\(item\) => toExternalImportResultViewModel\(item, extCopy\.result\)\)/,
  );
  assert.match(source, /appendExternalImportResults/);
  assert.match(
    source,
    /isExternalImportResultCoverageValid\(\s*page\.totalCount,\s*page\.nextCursor,\s*results\.length/,
  );
  assert.match(
    source,
    /isExternalImportResultCoverageValid\(\s*page\.totalCount,\s*page\.nextCursor,\s*mergedResults\.length/,
  );
  assert.match(
    source,
    /Number\(page\.nextCursor\)\s*<=\s*Number\(current\.nextCursor\)/,
  );
  assert.match(source, /refreshedTaskIdsRef\.current\.has\(taskId\)/);
  assert.match(source, /onImportedRef\.current\(\)/);
});

test("result retry sends only the sealed selection and reuses task progress launch", () => {
  const source = readSource(
    "src/features/mods/external-import/useExternalImportResultWorkflow.ts",
  );

  assert.match(source, /retryExternalImportBatch\(\{\s*batchId:\s*currentBatchId,\s*selectionId/);
  assert.match(source, /launchImport\(\(\)\s*=>/);
  assert.doesNotMatch(source, /candidateIds|decision|results:\s*current/);
  assert.match(source, /retryPendingRef\.current/);
  assert.match(source, /progressReady && canRetryState\(visibleState\)/);
  assert.match(source, /!progressReady/);
  assert.match(
    source,
    /currentState\.status === "ready" && currentState\.loadingMore/,
  );
  assert.match(
    source,
    /launchResult\.status === "ignored"[\s\S]{0,180}external_import_task_unavailable/,
  );
  assert.match(source, /const retryResults = useCallback/);
  assert.match(source, /const loadMore = useCallback/);
  assert.match(source, /const retryResultQuery = useCallback/);
});

test("selection workflow exposes a separate result workflow and library refresh callback", () => {
  const selection = readSource(
    "src/features/mods/external-import/useExternalImportSelectionWorkflow.ts",
  );
  const panel = readSource(
    "src/features/mods/external-import/ExternalImportSelectionPanel.tsx",
  );
  const action = readSource(
    "src/features/mods/external-import/ExternalImportAction.tsx",
  );
  const compactPanel = readSource("src/features/mods/CompactActionPanel.tsx");

  assert.match(selection, /useExternalImportResultWorkflow\(\{/);
  assert.match(selection, /onImported/);
  assert.match(selection, /result:\s*resultWorkflow/);
  assert.match(panel, /<ExternalImportResultPanel\s+workflow=\{workflow\.result\}/);
  assert.match(action, /onImported:\s*\(\)\s*=>\s*Promise<void>\s*\|\s*void/);
  assert.match(
    action,
    /useExternalImportSelectionWorkflow\([\s\S]*?batchId\s*:\s*null,\s*onImported/,
  );
  assert.match(compactPanel, /<ExternalImportAction\s+onImported=\{onImportCompleted\}/);
});
