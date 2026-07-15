import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing Task 8 source: ${path}`);
  return readFileSync(path, "utf8");
}

test("revision catalog query stays feature-local and accepts only a logical mod id", () => {
  const api = readSource("src/features/mods/modLibraryApi.ts");
  const types = readSource("src/features/mods/modLibraryTypes.ts");

  assert.match(api, /invoke<ModRevisionList>\("get_mod_revisions",\s*\{\s*modId:\s*input\.modId\s*\}\)/);
  assert.match(types, /export type ModRevisionList/);
  assert.match(types, /originRevisionId:\s*string/);
  assert.match(types, /displayRevisionId:\s*string/);
  assert.match(types, /revisions:\s*ModRevisionSummary\[\]/);
  assert.doesNotMatch(api, /archivePath|sourcePath|sandbox|cache|targetPath|manifest/i);
});

test("reinstall wrappers pass only controlled ids, layer, and preview token", () => {
  const api = readSource("src/features/mods/modReinstallApi.ts");

  assert.match(api, /invoke<ReinstallPlanPreview>\("preview_reinstall_plan"/);
  assert.match(api, /invoke<TaskStartedDto>\("start_reinstall_task"/);
  for (const field of ["gameId", "profileId", "modId", "candidateRevisionId", "layer"]) {
    assert.match(api, new RegExp(`${field}:\\s*input\\.${field}`));
  }
  assert.match(api, /planToken:\s*input\.planToken/);
  assert.doesNotMatch(api, /targetPath|deletePath|archivePath|sourcePath|sandbox|cache|backupRef|manifest|gameRoot|hash/i);
});

test("reinstall preview types form a strict ready or blocked discriminated union", () => {
  const source = readSource("src/features/mods/modReinstallTypes.ts");

  assert.match(source, /status:\s*"ready"/);
  assert.match(source, /planToken:\s*string/);
  assert.match(source, /installedRevision:\s*ModRevisionSummary/);
  assert.match(source, /candidateRevision:\s*ModRevisionSummary/);
  assert.match(source, /blockingReasons:\s*\[\]/);
  assert.match(source, /status:\s*"blocked"/);
  assert.match(source, /planToken:\s*null/);
  assert.match(source, /installedRevision:\s*ModRevisionSummary\s*\|\s*null/);
  assert.match(source, /candidateRevision:\s*ModRevisionSummary\s*\|\s*null/);
  assert.match(source, /"candidate_not_found"/);
  assert.match(source, /"preview_stale"/);
  assert.doesNotMatch(source, /planToken\?:|candidateRevision\?:|installedRevision\?:/);
});

test("reinstall preview panel narrows ready state before confirming and renders all count classes", () => {
  const source = readSource("src/features/mods/ReinstallPlanPreviewPanel.tsx");

  assert.match(source, /preview\.status\s*===\s*"ready"/);
  assert.match(source, /preview\.status\s*===\s*"blocked"/);
  assert.match(source, /retained/);
  assert.match(source, /replaced/);
  assert.match(source, /added/);
  assert.match(source, /stale/);
  assert.match(source, /candidate_not_found/);
  assert.match(source, /preview_stale/);
  assert.match(source, /candidateRevision\s*\?\s*/);
  assert.doesNotMatch(source, /candidateRevision!|installedRevision!|planToken!/);
  assert.match(source, /role="dialog"/);
  assert.match(source, /aria-modal="true"/);
  assert.match(source, /getTrappedFocusIndex/);
});

test("mod library routes install and true reinstall through separate commands", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  const panel = readSource("src/features/mods/CompactActionPanel.tsx");
  const data = readSource("src/features/mods/modsLibraryData.ts");

  assert.match(page, /case\s+"install":[\s\S]*?startSelectedInstallTask\(\)/);
  assert.match(page, /case\s+"reinstall":[\s\S]*?openReinstall/);
  assert.doesNotMatch(page, /case\s+"reinstall":\s*startSelectedInstallTask\(\)/);
  assert.match(data, /id:\s*"install"/);
  assert.match(data, /id:\s*"reinstall"/);
  assert.match(panel, /canReinstallSelection/);
  assert.match(panel, /selectedModId/);
});

test("reinstall workflow matches task id and phase then refetches durable facts for every terminal state", () => {
  const source = readSource("src/features/mods/useModReinstallWorkflow.ts");

  assert.match(source, /event\.payload\.taskId\s*!==/);
  assert.match(source, /isReinstallTaskPhase\(event\.payload\.phase\)/);
  assert.match(source, /pendingProgressEventsRef/);
  assert.match(source, /isReinstallTaskTerminal/);
  assert.match(source, /refreshReinstallDurableFacts/);
  assert.match(source, /getModRevisions/);
  assert.match(source, /refreshLibrary/);
  assert.doesNotMatch(source, /startInstallTask/);
});

test("post-commit failure copy is fail-closed and never offers a v1 rollback shortcut", () => {
  const source = readSource("src/features/mods/ReinstallPlanPreviewPanel.tsx");

  assert.match(source, /committed_cleanup_pending/);
  assert.match(source, /cleanup_pending/);
  assert.match(source, /新版本已提交/);
  assert.doesNotMatch(source, /回滚到\s*(?:v1|旧版本)|rollback.*v1/i);
  assert.doesNotMatch(source, /安装状态已刷新/);
});
