import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("batch lifecycle API invokes the six documented narrow commands", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/batchModLifecycleApi.ts",
  );

  assert.match(source, /invoke<BatchModLifecycleCapabilityDto>\("get_batch_mod_lifecycle_capability"\)/);
  assert.match(source, /invoke<BatchModLifecyclePreviewDto>\("preview_batch_mod_lifecycle"/);
  assert.match(source, /request,\s*\n\s*\}\);/);
  assert.match(source, /invoke<BatchModLifecycleSealDto>\("seal_batch_mod_lifecycle"/);
  assert.match(source, /previewToken:\s*input\.previewToken/);
  assert.match(source, /invoke<BatchModLifecycleStartedDto>\("start_batch_mod_lifecycle"/);
  assert.match(source, /batchId:\s*input\.batchId/);
  assert.match(source, /planToken:\s*input\.planToken/);
  assert.match(source, /invoke<BatchModLifecycleResultPageDto>\(\s*"get_batch_mod_lifecycle_result"/);
  assert.match(source, /attemptNumber:\s*input\.attemptNumber/);
  assert.match(source, /cursor:\s*input\.cursor\s*\?\?\s*null/);
  assert.match(source, /limit:\s*input\.limit\s*\?\?\s*BATCH_MOD_LIFECYCLE_RESULT_PAGE_SIZE/);
  assert.match(source, /invoke<BatchModLifecycleStartedDto>\("retry_batch_mod_lifecycle"/);
  assert.match(source, /expectedAttemptNumber:\s*input\.expectedAttemptNumber/);
  assert.match(source, /invoke<TaskStartedDto>\("cancel_task"/);
  assert.match(source, /taskId:\s*input\.taskId/);
});

test("batch lifecycle API never reaches for filesystem or path primitives", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/batchModLifecycleApi.ts",
  );

  assert.doesNotMatch(
    source,
    /readFile|writeFile|removeFile|convertFileSrc|asset:|thumbnail:|nativePC|installPath|targetPath|cachePath|sandboxPath|archivePath/i,
  );
});

test("batch lifecycle API does not fabricate batch semantics", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/batchModLifecycleApi.ts",
  );

  assert.doesNotMatch(
    source,
    /replacementBindingSnapshot|planToken\s*=|previewToken\s*=|manifest|backupRef|digest|hash/i,
  );
  assert.doesNotMatch(source, /for\s*\(/);
  assert.doesNotMatch(source, /\.map\(/);
});

test("batch lifecycle types register stable status and policy vocabularies", () => {
  const source = readSource(
    "src/features/mods/batch-lifecycle/batchModLifecycleTypes.ts",
  );

  assert.match(source, /BATCH_MOD_LIFECYCLE_RESULT_PAGE_SIZE\s*=\s*50/);
  assert.match(source, /BATCH_MOD_LIFECYCLE_RESULT_PAGE_MAX_SIZE\s*=\s*100/);
  assert.match(source, /BATCH_MOD_LIFECYCLE_MAX_ITEMS\s*=\s*100/);
  assert.match(source, /BATCH_MOD_LIFECYCLE_SCHEMA_VERSION\s*=\s*1/);
  assert.match(source, /BatchModLifecycleCapabilityDto = \{\s*previewAvailable: boolean;/);
  assert.match(source, /unavailableReasonCode: string \| null/);
  assert.match(source, /"stop_on_failure"/);
  assert.match(source, /"continue_on_item_failure"/);
  assert.match(source, /"completed_with_errors"/);
  assert.match(source, /"recovery_required"/);
  assert.match(source, /"interrupted"/);
  assert.match(source, /"succeeded"/);
  assert.match(source, /"skipped"/);

  assert.doesNotMatch(
    source,
    /installPath|targetPath|nativePC|manifest|backupRef|digest|cachePath|sandboxPath/i,
  );
});
