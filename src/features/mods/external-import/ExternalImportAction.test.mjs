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

test("external import action composes selection, result, and retry without a frontend file picker", () => {
  const source = readSource("src/features/mods/external-import/ExternalImportAction.tsx");
  const selectionPanel = readSource(
    "src/features/mods/external-import/ExternalImportSelectionPanel.tsx",
  );
  const selectionWorkflow = readSource(
    "src/features/mods/external-import/useExternalImportSelectionWorkflow.ts",
  );
  const progressState = readSource(
    "src/features/mods/external-import/externalImportProgressState.ts",
  );
  const candidateSelection = readSource(
    "src/features/mods/external-import/ExternalImportCandidateSelectionItem.tsx",
  );
  const resultPanel = readSource(
    "src/features/mods/external-import/ExternalImportResultPanel.tsx",
  );
  const styles = readSource(
    "src/features/mods/external-import/ExternalImportAction.css",
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
  assert.match(selectionWorkflow, /useExternalImportTaskProgress\(batchId\)/);
  assert.doesNotMatch(selectionWorkflow, /listen<TaskProgressEventDto>/);
  assert.match(progressState, /importPhases\.has\(event\.phase\)/);
  assert.match(progressState, /event\.phase === "mod_import\.cancelled"/);
  assert.match(progressState, /external_import\.import\./);
  assert.match(selectionWorkflow, /const runLoadMore = useCallback/);
  assert.match(
    selectionWorkflow,
    /const\s*\{[\s\S]{0,240}launchImport,[\s\S]{0,160}cancelImport,[\s\S]{0,80}\}\s*=\s*useExternalImportTaskProgress\(batchId\)/,
  );
  assert.doesNotMatch(selectionWorkflow, /taskProgress\./);
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
  assert.match(
    panelSource,
    /<ExternalImportAction onImported=\{onImportCompleted\} disabledReason=\{storageWriteFreezeReason\} \/>/,
  );
  assert.doesNotMatch(source, /@tauri-apps\/plugin-dialog|open\(\{/);
  assert.match(selectionWorkflow, /useExternalImportResultWorkflow/);
  assert.match(selectionPanel, /ExternalImportResultPanel/);
  assert.match(resultPanel, /<section[^>]*aria-labelledby=\{headingId\}/);
  assert.match(resultPanel, /<h3 id=\{headingId\}/);
  assert.match(resultPanel, /role="status" aria-live="polite"/);
  assert.match(resultPanel, /role="alert"/);
  assert.match(resultPanel, /<ul className="external-import__result-list">/);
  assert.match(resultPanel, /disabled=\{state\.loadingMore/);
  assert.match(
    resultPanel,
    /state\.status === "ready" && state\.loadingMore[\s\S]{0,120}workflow\.retryPending[\s\S]{0,80}workflow\.resultStale/,
  );
  assert.match(styles, /clip-path:\s*inset\(50%\)/);
  assert.doesNotMatch(styles, /\bclip:\s*rect\(/);
  assert.match(
    selectionWorkflow,
    /launchResult\.status === "ignored"[\s\S]{0,180}external_import_task_unavailable/,
  );
  assert.match(
    resultPanel,
    /resultPanel\.reloadResults|resultPanel\.loadMoreResults|resultPanel\.retryRecoverable/,
  );
  // 结果行以候选显示名为主标题(缺省走未命名兜底),candidateId 降级为次要 code。
  assert.match(
    resultPanel,
    /<strong>\{result\.displayName \?\? extCopy\.preview\.unnamed\}<\/strong>/,
  );
  assert.doesNotMatch(resultPanel, /<strong>\{extCopy\.resultPanel\.candidateResult\}<\/strong>/);
  // 弹窗内「本次导入 / 导入记录」页签;记录模式打开绝不拉起原生目录选择器。
  assert.match(source, /role="tablist"/);
  assert.match(source, /view === "history"/);
  assert.match(source, /<ExternalImportHistoryPanel workflow=\{historyWorkflow\} \/>/);
  assert.match(source, /function openHistory\(\) \{[\s\S]{0,220}ensureLoaded\(\);[\s\S]{0,20}\}/);
  assert.doesNotMatch(source, /function openHistory\(\) \{[\s\S]{0,300}chooseSource/);
  // 工具栏记录直达入口只打开记录视图,不承担导入职责。
  assert.match(source, /external-import-action__history-trigger/);
  assert.match(source, /onClick=\{openHistory\}/);
  // 入口按钮工具栏治理:服务状态走 tooltip + 警示点,可访问播报走隐藏 live region,
  // 工具栏不出现常驻红字。
  assert.match(source, /<ModLibraryControlTooltip content=\{triggerStatusText\}>/);
  assert.match(source, /data-listener-status=\{listenerStatus\}/);
  assert.match(
    source,
    /listenerStatus === "failed" \? \(\s*<span className="compact-import-action__alert-dot" aria-hidden="true" \/>/,
  );
  assert.match(
    source,
    /className="compact-import-action__sr-status"\s*role=\{listenerStatus === "failed" \? "alert" : "status"\}/,
  );
  // 误选目录(如狩技盒子安装根的父级)引导:0 项可导入时给定向文案。必须等分页取完
  // (nextCursor === null)才敢下结论,否则第 2 页才出现的 ready 候选会把玩家误导去重选目录。
  assert.match(selectionPanel, /selectionPanel\.noImportableHint/);
  assert.match(
    selectionPanel,
    /candidates\.length > 0 &&\s*workflow\.previewState\.nextCursor === null &&/,
  );
  assert.match(
    selectionPanel,
    /previewStatus === "ready" \|\|[\s\S]{0,120}previewStatus === "metadata_invalid"/,
  );
  // 导入完成后的去处引导:查看记录会强制刷新历史,让刚完成的批次立刻可见。
  assert.match(selectionPanel, /selectionPanel\.viewImportHistory/);
  assert.match(selectionPanel, /selectionPanel\.closeAndReturn/);
  assert.match(source, /onViewHistory=\{\(\) => \{\s*setView\("history"\);[\s\S]{0,120}historyWorkflow\.refresh\(\);/);
  assert.match(source, /onCloseDialog=\{\(\) => setDialogOpen\(false\)\}/);
  assert.doesNotMatch(
    `${source}\n${selectionPanel}\n${candidateSelection}\n${resultPanel}\n${selectionWorkflow}`,
    /readFile|writeFile|removeFile|convertFileSrc|asset:|thumbnail:|sandbox|cache|archivePath/i,
  );
});
