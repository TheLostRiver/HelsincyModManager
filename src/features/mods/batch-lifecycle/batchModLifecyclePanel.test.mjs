import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  batchModLifecycleCopy,
  getBatchAttemptStatusLabel,
  getBatchCapabilityUnavailableLabel,
  getBatchErrorLabel,
  getBatchExcludedReasonLabel,
  getBatchItemStatusLabel,
  getBatchOperationLabel,
} from "./batchModLifecycleCopy.ts";

const zhBatch = batchModLifecycleCopy.zh_cn;

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("batch preview panel exposes a modal with policy choice and blocked confirmation", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/BatchModLifecyclePreviewPanel.tsx",
  );

  assert.match(source, /role="dialog"/);
  assert.match(source, /aria-modal="true"/);
  assert.match(source, /aria-labelledby=\{titleId\}/);
  assert.match(source, /useModalFocusTrap\(/);
  assert.match(source, /closeOnEscape: true/);
  assert.match(source, /type="radio"/);
  assert.match(source, /stop_on_failure/);
  assert.match(source, /continue_on_item_failure/);
  assert.match(source, /panelCopy\.stopOnFailure/);
  assert.match(source, /panelCopy\.continueOnFailure/);
  assert.match(source, /preview\.previewToken === null/);
  assert.match(source, /blockedItemCount > 0/);
  assert.match(source, /target-selection/);
  assert.match(source, /onReplacementTargetChange/);
  assert.match(source, /onPreviewWithReplacementTargets/);
  assert.match(source, /type="radio"/);
  assert.match(source, /getBatchOperationLabel\(operation, bCopy\.operations\)/);
  assert.match(source, /aria-label=\{panelCopy\.itemsAria\}/);
  assert.match(source, /panelCopy\.installedRevision/);
  assert.match(source, /panelCopy\.candidateDisplayRevision/);
  assert.match(source, /item\.layer\.name/);
  assert.match(source, /item\.layer\.priority/);
  assert.match(source, /panelCopy\.switchTo\(/);
  assert.match(source, /replacementTargets/);
  assert.doesNotMatch(source, /nativePC|targetPath|installPath|cachePath|sandboxPath|convertFileSrc/i);
});

test("batch panels consume appearance tokens and keep controls reachable on narrow windows", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/BatchModLifecyclePanel.css",
  );

  assert.match(source, /var\(--color-surface\)/);
  assert.match(source, /var\(--color-text\)/);
  assert.match(source, /var\(--color-border\)/);
  assert.match(source, /var\(--z-feedback-dialog\)/);
  assert.match(source, /\.batch-panel__body/);
  assert.match(source, /overflow-y:\s*auto/);
  assert.match(source, /@media \(max-width: 640px\)/);
  assert.match(source, /100dvh/);
  assert.doesNotMatch(source, /--hmm-/);
  assert.doesNotMatch(source, /#1c1f26|#e8e8ea|#f0a0a0|#e8c080|#90d8a8/i);
});

test("batch result panel gates retry and pagination on backend facts", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/BatchModLifecycleResultPanel.tsx",
  );

  assert.match(source, /retryAvailableByStatus =/);
  assert.match(source, /result\.status === "completed_with_errors"/);
  assert.match(source, /result\.status === "failed"/);
  assert.match(source, /retryAvailableByStatus &&/);
  assert.match(source, /panelCopy\.retryFailed/);
  assert.match(source, /canLoadMore = result\.nextCursor !== null/);
  assert.match(source, /panelCopy\.loadMore/);
  assert.match(source, /getBatchItemStatusLabel\(item\.status, bCopy\.itemStatus\)/);
  assert.match(source, /getBatchReasonCodeLabel\(item\.reasonCode, bCopy\.reasonCodes\)/);
  assert.match(source, /evidenceHealthDegraded/);
  assert.doesNotMatch(source, /nativePC|targetPath|installPath|cachePath|sandboxPath|convertFileSrc/i);
});

test("batch copy maps stable codes without raw backend text", () => {
  assert.equal(getBatchOperationLabel("install", zhBatch.operations), "批量安装");
  assert.equal(getBatchOperationLabel("uninstall", zhBatch.operations), "批量卸载");
  assert.equal(getBatchOperationLabel("reinstall", zhBatch.operations), "批量重装");
  assert.equal(getBatchItemStatusLabel("succeeded", zhBatch.itemStatus), "成功");
  assert.equal(getBatchItemStatusLabel("recovery_required", zhBatch.itemStatus), "需要恢复");
  assert.equal(getBatchItemStatusLabel("skipped", zhBatch.itemStatus), "已跳过");
  assert.equal(getBatchAttemptStatusLabel("completed_with_errors", zhBatch.attemptStatus), "部分成功");
  assert.equal(getBatchAttemptStatusLabel("interrupted", zhBatch.attemptStatus), "已中断");

  const codes = [
    "batch_no_applicable_items",
    "batch_facts_unavailable",
    "batch_replacement_facts_unavailable",
    "batch_input_invalid",
    "batch_duplicate_item",
    "batch_resource_limit_exceeded",
    "batch_global_target_conflict",
    "batch_plan_blocked",
    "batch_plan_stale",
    "batch_plan_expired",
    "batch_token_invalid",
    "batch_retry_unavailable",
    "batch_attempt_stale",
    "batch_result_unavailable",
    "batch_journal_unavailable",
    "batch_evidence_unavailable",
    "sandbox_batch_production_forbidden",
    "batch_internal_error",
  ];
  for (const code of codes) {
    const label = getBatchErrorLabel(code, zhBatch);
    assert.ok(label.length > 0, `missing copy for ${code}`);
    assert.ok(!label.includes(":"), `${code} copy must not embed backend text`);
    assert.ok(!label.includes("\\"), `${code} copy must not embed paths`);
  }
  assert.equal(getBatchErrorLabel("unknown_code", zhBatch), "批量操作失败");
  assert.equal(getBatchExcludedReasonLabel("already_installed", zhBatch), "已安装，不参与本次安装");
  assert.equal(
    getBatchExcludedReasonLabel("installed_revision_unavailable", zhBatch),
    "已安装但缺少版本信息（旧格式清单），无法参与",
  );
});

test("ModLibraryPage dispatches lifecycle flows by explicit selection mode", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(page, /case "install":\s*if \(selectionMode === "single"\) \{\s*startSelectedInstallTask\(\);\s*\} else if \(!selectionInteractionLocked && batchWriteAvailable\) \{\s*void batchWorkflow\.prepare\("install"/);
  assert.match(page, /case "uninstall":\s*if \(selectionMode === "single"\) \{\s*promptSelectedUninstallTask\(\);\s*\} else if \(!selectionInteractionLocked && batchWriteAvailable\) \{\s*void batchWorkflow\.prepare\("uninstall"/);
  assert.match(page, /case "reinstall":[\s\S]*?if \(selectionMode === "single"\) \{\s*openReinstall\(\);\s*\} else if \(!selectionInteractionLocked && batchWriteAvailable\) \{\s*void batchWorkflow\.prepare\("reinstall"/);
  assert.match(page, /selectionMode !== "single"\s*\|\| selectedIds\.size !== 1/);
  assert.match(page, /batchWorkflow\.state\.status === "result" &&/);
  assert.match(page, /batchWorkflow\.state\.status === "starting"/);
  assert.match(page, /BatchModLifecyclePreviewPanel/);
  assert.match(page, /BatchModLifecycleResultPanel/);
  assert.match(page, /useEffect\(\(\) => \{\s*batchWorkflow\.reset\(\);/);
  assert.match(page, /handledBatchTerminalAttemptsRef = useRef\(new Set<string>\(\)\)/);
  assert.match(page, /batchWorkflow\.state\.status !== "result"/);
  assert.match(
    page,
    /const batchAttemptKey = `\$\{batchWorkflow\.state\.batchId\}:\$\{batchWorkflow\.state\.attemptNumber\}`/,
  );
  assert.match(
    page,
    /handledBatchTerminalAttemptsRef\.current\.add\(batchAttemptKey\);\s*void refreshLibraryPage\(\)\.catch/,
  );
});

test("batch lifecycle resolves exact installed revisions from manifest facts", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const batchWorkflowCall = page.match(
    /const batchWorkflow = useBatchModLifecycleWorkflow\(\{[\s\S]*?\n {2}\}\);/,
  );
  assert.ok(batchWorkflowCall, "expected batch workflow wiring");

  const manifestLoader = batchWorkflowCall[0].match(
    /loadManifestStatuses:[\s\S]*?loadRevisions:/,
  );
  assert.ok(manifestLoader, "expected batch manifest loader");
  assert.match(manifestLoader[0], /getInstallManifestStatus\(\{/);
  assert.match(manifestLoader[0], /profileId:\s*profileContext\.profileId/);
  assert.match(manifestLoader[0], /modIds/);
  assert.doesNotMatch(
    manifestLoader[0],
    /gameId:/,
    "gameId switches the command to recovery projection, which has no installed revision",
  );
});

test("batch panels keep selection-invalidation wiring", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  // 选择变化使旧 batch plan 失效（T13-07 契约）。
  assert.match(page, /Selection changes invalidate any in-flight batch preview/);
  assert.match(page, /batchWorkflow\.reset\(\);\s*\/\/ eslint-disable-next-line/);
});

test("batch capability is backend-owned, fail-closed, and mapped to product copy", () => {
  const hookSource = readSource(
    "src/features/mods/batch-lifecycle/useBatchModLifecycleCapability.ts",
  );

  assert.match(hookSource, /status: "loading",\s*capability: null/);
  assert.match(hookSource, /getBatchModLifecycleCapability\(\)/);
  assert.match(hookSource, /catch\(\(\) =>/);
  assert.match(hookSource, /UNAVAILABLE_BATCH_CAPABILITY/);
  assert.doesNotMatch(hookSource, /HMM_SANDBOX_DATA_DIR|process\.env|import\.meta\.env/);

  assert.equal(
    getBatchCapabilityUnavailableLabel(
      {
        previewAvailable: false,
        writeAvailable: false,
        unavailableReasonCode: "sandbox_batch_production_forbidden",
      },
      zhBatch.capability,
    ),
    "当前版本仅允许在受控测试环境执行批量操作",
  );
  assert.equal(
    getBatchCapabilityUnavailableLabel(
      {
        previewAvailable: false,
        writeAvailable: false,
        unavailableReasonCode: "batch_capability_unavailable",
      },
      zhBatch.capability,
    ),
    "无法确认批量操作权限，请刷新后重试",
  );
  assert.ok(getBatchCapabilityUnavailableLabel(null, zhBatch.capability).length > 0);
});

test("batch replacement target names are projected per render, not frozen at load", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const panel = readSource(
    "src/features/mods/batch-lifecycle/BatchModLifecyclePreviewPanel.tsx",
  );
  const types = readSource(
    "src/features/mods/batch-lifecycle/batchModLifecycleTypes.ts",
  );

  // The facts carry the raw multi-locale names. Resolving them while loading would
  // bake one locale into the workflow state, so a language switch mid-flow would
  // leave the dropdown stale until the facts are refetched (which would also drop
  // the current selection). This is the I18N-08 contract.
  assert.match(
    types,
    /BatchModLifecycleReplacementTargetOption = \{\s*id: string;\s*displayNames: ReplacementTargetDisplayNames;/,
  );
  assert.doesNotMatch(
    types,
    /BatchModLifecycleReplacementTargetOption = \{[^}]*\bdisplayName\b/,
    "target options must not carry a pre-resolved display name",
  );
  assert.match(
    page,
    /targets: targets\.map\(\(\{ id, displayNames \}\) => \(\{\s*id,\s*displayNames,\s*\}\)\)/,
  );
  assert.doesNotMatch(
    page,
    /resolveReplacementTargetNames\(/,
    "ModLibraryPage must not resolve replacement target names at load time",
  );

  // The panel projects them on every render, so switching the UI language relabels
  // the options in place — no refetch, no state loss.
  assert.match(
    panel,
    /availableTargets\.map\(\(target\) => \{[\s\S]{0,400}?resolveReplacementTargetNames\(\s*target\.displayNames,\s*locale,\s*\)/,
  );
  assert.match(panel, /<strong>\{displayName\}<\/strong>/);
  assert.match(panel, /\{secondaryName \? <small>\{secondaryName\}<\/small> : null\}/);
});
