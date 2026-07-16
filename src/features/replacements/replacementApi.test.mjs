import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

function exportedFunction(source, name) {
  const match = source.match(new RegExp(`export function ${name}\\b[\\s\\S]*?\\n}`));
  assert.ok(match, `expected exported function ${name}`);
  return match[0];
}

function inlineRequestEntries(functionSource) {
  const request = functionSource.match(/request:\s*\{([\s\S]*?)\}\s*,?\s*\}\s*\)/);
  assert.ok(request, "expected inline request object");
  return [...request[1].matchAll(/^\s*([a-zA-Z][a-zA-Z0-9]*):\s*([^,\n]+)/gm)].map(
    (match) => `${match[1]}: ${match[2].trim()}`,
  );
}

test("replacement typed API wrappers use exact commands and request shapes", () => {
  assert.equal(existsSync("src/features/replacements/replacementApi.ts"), true);
  assert.equal(existsSync("src/features/replacements/replacementTypes.ts"), true);
  const api = readSource("src/features/replacements/replacementApi.ts");
  const types = readSource("src/features/replacements/replacementTypes.ts");

  const list = exportedFunction(api, "listReplacementTargets");
  assert.match(list, /invoke<ReplacementTarget\[\]>\("list_replacement_targets"/);
  assert.deepEqual(inlineRequestEntries(list), ["gameId: input.gameId", "query: input.query"]);

  const analyze = exportedFunction(api, "analyzeImportedModReplacement");
  assert.match(analyze, /invoke<ReplacementAnalysis>\("analyze_imported_mod_replacement"/);
  assert.deepEqual(inlineRequestEntries(analyze), [
    "gameId: input.gameId",
    "modId: input.modId",
  ]);

  const preview = exportedFunction(api, "previewInitialRetargetInstall");
  assert.match(preview, /invoke<InitialRetargetInstallPreview>\("preview_initial_retarget_install"/);
  assert.match(preview, /\{\s*request:\s*initialRetargetRequest\(input\),?\s*\}/);

  const start = exportedFunction(api, "startRetargetInstallTask");
  assert.match(start, /invoke<RetargetInstallTaskStarted>\("start_retarget_install_task"/);
  assert.match(start, /\{\s*request:\s*initialRetargetRequest\(input\),?\s*\}/);

  const sharedRequest = api.match(/function initialRetargetRequest\b[\s\S]*?return\s*\{([\s\S]*?)\};\s*\n}/);
  assert.ok(sharedRequest, "expected shared initial retarget request mapper");
  assert.deepEqual(
    [...sharedRequest[1].matchAll(/^\s*([a-zA-Z][a-zA-Z0-9]*):\s*([^,\n]+)/gm)].map(
      (match) => `${match[1]}: ${match[2].trim()}`,
    ),
    [
      "gameId: input.gameId",
      "profileId: input.profileId",
      "modId: input.modId",
      "targetId: input.targetId",
      "layerName: input.layerName",
      "layerPriority: input.layerPriority",
    ],
  );
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
