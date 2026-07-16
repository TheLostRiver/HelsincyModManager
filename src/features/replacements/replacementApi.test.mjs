import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("replacement typed API invokes the four controlled AR4 commands", () => {
  assert.equal(existsSync("src/features/replacements/replacementApi.ts"), true);
  assert.equal(existsSync("src/features/replacements/replacementTypes.ts"), true);
  const api = readSource("src/features/replacements/replacementApi.ts");
  const types = readSource("src/features/replacements/replacementTypes.ts");

  for (const command of [
    "list_replacement_targets",
    "analyze_imported_mod_replacement",
    "preview_initial_retarget_install",
    "start_retarget_install_task",
  ]) {
    assert.match(api, new RegExp(`invoke<[^>]+>\\("${command}"`));
  }
  for (const field of ["gameId", "modId", "profileId", "targetId", "layerName", "layerPriority"]) {
    assert.match(api, new RegExp(`${field}:\\s*input\\.${field}`));
  }
  assert.match(types, /export type ReplacementTarget/);
  assert.match(types, /export type ReplacementAnalysis/);
  assert.match(types, /export type InitialRetargetInstallPreview/);
  assert.doesNotMatch(
    api,
    /packageId|revisionId|sourceId|bindingId|sandbox|staging|gameRoot|archivePath|rawPath/i,
  );
});

test("replacement request types expose stable ids but no filesystem or package facts", () => {
  const source = readSource("src/features/replacements/replacementTypes.ts");
  const requestTypes = source.match(
    /export type ListReplacementTargetsInput[\s\S]*?export type ReplacementTarget/,
  );
  assert.ok(requestTypes, "expected request type block");
  for (const field of ["gameId", "modId", "profileId", "targetId", "layerName", "layerPriority"]) {
    assert.match(requestTypes[0], new RegExp(`${field}`));
  }
  assert.doesNotMatch(
    requestTypes[0],
    /packageId|revisionId|sourceId|bindingId|sandbox|staging|targetPath|archivePath|rawPath/i,
  );
});
