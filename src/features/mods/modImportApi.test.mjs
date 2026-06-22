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

test("mod import task types expose controlled task identity and archive path", () => {
  const source = readSource("src/features/mods/modImportTypes.ts");

  assert.match(source, /archivePath:\s*string/);
  assert.match(source, /taskId:\s*string/);
  assert.match(source, /kind:\s*"mod_import"/);
  assert.match(source, /status:\s*"queued"\s*\|\s*"running"\s*\|\s*"completed"\s*\|\s*"failed"\s*\|\s*"cancelled"/);
  assert.doesNotMatch(source, /previewImage|thumbnailUrl|sandbox|cache/i);
});
