import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(new URL("./profileSaveRestoreApi.ts", import.meta.url), "utf8");

test("save restore API uses narrow preview, start, and cancellation commands", () => {
  assert.match(source, /invoke<SaveRestorePreviewDto>\("preview_save_restore"/);
  assert.match(source, /invoke<SaveRestoreTaskStartedDto>\("start_save_restore_task"/);
  assert.match(source, /invoke<TaskStartedDto>\("cancel_task", \{ taskId \}\)/);
  assert.match(source, /confirmed:\s*true/);
});

test("save restore API never accepts filesystem or manifest facts", () => {
  assert.doesNotMatch(
    source,
    /saveDirectory|backupDirectory|archivePath|manifestPath|targetPath|sourcePath|fileList|archiveSha256/,
  );
});
