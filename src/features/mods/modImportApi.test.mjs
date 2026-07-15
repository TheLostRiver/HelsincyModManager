import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("mod import API invokes the controlled task entry command", () => {
  const source = readSource("src/features/mods/modImportApi.ts");

  assert.match(source, /invoke<TaskStartedDto>\("start_import_mod_task"/);
  assert.match(source, /archivePath:\s*input\.archivePath/);
  assert.doesNotMatch(source, /convertFileSrc|asset:|thumbnail:|read_image_path/);
});

test("mod revision import API attaches a picker archive to an explicit logical mod", () => {
  const source = readSource("src/features/mods/modImportApi.ts");

  assert.match(source, /invoke<TaskStartedDto>\("start_import_mod_revision_task"/);
  const call = source.match(/start_import_mod_revision_task[\s\S]*?request:\s*\{([\s\S]*?)\}\s*,?\s*\}\s*\)/);
  assert.ok(call, "expected revision import to use the request DTO boundary");
  assert.match(call[1], /archivePath:\s*input\.archivePath/);
  assert.match(call[1], /modId:\s*input\.modId/);
  assert.doesNotMatch(call[1], /displayName|author|version|targetPath|sandbox|cache/i);
});

test("mod import API invokes the controlled cancel task command", () => {
  const source = readSource("src/features/mods/modImportApi.ts");

  assert.match(source, /invoke<TaskStartedDto>\("cancel_task"/);
  assert.match(source, /taskId:\s*input\.taskId/);
  assert.doesNotMatch(source, /archivePath.*cancel|sandbox|cache|rawPath/i);
});

test("mod import task types expose controlled task identity and archive path", () => {
  const source = readSource("src/features/mods/modImportTypes.ts");

  assert.match(source, /archivePath:\s*string/);
  assert.match(source, /export type StartImportModRevisionTaskInput/);
  assert.match(source, /modId:\s*string/);
  assert.match(source, /export type CancelTaskInput/);
  assert.match(source, /taskId:\s*string/);
  assert.match(source, /export type TaskKind\s*=\s*"mod_import"\s*\|\s*"install"/);
  assert.match(source, /kind:\s*TaskKind/);
  assert.match(source, /export type TaskStatus\s*=\s*"queued"\s*\|\s*"running"\s*\|\s*"completed"\s*\|\s*"failed"\s*\|\s*"cancelled"/);
  assert.match(source, /status:\s*TaskStatus/);
  assert.doesNotMatch(source, /previewImage|thumbnailUrl|sandbox|cache/i);
});

test("mod import task event types mirror the backend progress payload", () => {
  const source = readSource("src/features/mods/modImportTypes.ts");

  assert.match(source, /export type TaskProgressEventDto/);
  assert.match(source, /phase:\s*string/);
  assert.match(source, /current:\s*number\s*\|\s*null/);
  assert.match(source, /total:\s*number\s*\|\s*null/);
  assert.match(source, /message:\s*string\s*\|\s*null/);
  assert.match(source, /error:\s*string\s*\|\s*null/);
  assert.match(source, /resultRef:\s*string\s*\|\s*null/);
  assert.match(source, /export const TASK_PROGRESS_EVENT_NAME\s*=\s*"hmm:\/\/task-progress"/);
  assert.doesNotMatch(source, /rawPath|archivePath.*TaskProgressEventDto|sandbox|cache/i);
});
