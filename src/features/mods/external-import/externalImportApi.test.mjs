import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("external import API only invokes the documented source, scan, preview, and cancel boundaries", () => {
  const source = readSource("src/features/mods/external-import/externalImportApi.ts");

  assert.match(source, /invoke<ExternalImportSourceDto \| null>\("select_external_import_source"\)/);
  assert.match(source, /invoke<ExternalImportScanStartedDto>\("start_external_import_scan"/);
  assert.match(source, /sourceId:\s*input\.sourceId/);
  assert.match(source, /invoke<ExternalImportPreviewPageDto>\("get_external_import_preview"/);
  assert.match(source, /batchId:\s*input\.batchId/);
  assert.match(source, /cursor:\s*input\.cursor\s*\?\?\s*null/);
  assert.match(source, /limit:\s*EXTERNAL_IMPORT_PREVIEW_PAGE_SIZE/);
  assert.match(source, /invoke<TaskStartedDto>\("cancel_task"/);
  assert.match(source, /taskId:\s*input\.taskId/);
  assert.doesNotMatch(source, /create_external_import_selection|update_external_import_selection|select_all_external_import_candidates/);
  assert.doesNotMatch(source, /start_external_import_batch|retry_external_import_batch|get_external_import_batch_result/);
  assert.doesNotMatch(source, /readFile|writeFile|removeFile|convertFileSrc|asset:|thumbnail:|sandbox|cache|archivePath/i);
});
