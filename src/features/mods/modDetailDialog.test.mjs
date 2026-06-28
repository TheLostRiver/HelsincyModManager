import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readProjectFile(relativePath) {
  const path = join(repoRoot, relativePath);
  assert.ok(existsSync(path), `missing file: ${relativePath}`);
  return readFileSync(path, "utf8");
}

test("mod detail dialog edits metadata and category assignments through controlled APIs", () => {
  const source = readProjectFile("src/features/mods/ModDetailDialog.tsx");

  assert.match(source, /updateModMetadata/);
  assert.match(source, /listCategories/);
  assert.match(source, /getModCategories/);
  assert.match(source, /setModCategories/);
  assert.match(source, /getModDetail/);
  assert.match(source, /type="checkbox"/);
  assert.match(source, /name="displayName"/);
  assert.match(source, /name="author"/);
  assert.match(source, /name="version"/);
  assert.match(source, /name="nexusModId"/);
  assert.match(source, /name="description"/);
  assert.match(source, /className="mod-detail-dialog__preview"/);
  assert.doesNotMatch(source, /convertFileSrc|asset:|archivePath|sandbox|cachePath|rawPath/i);
});

test("mod library page opens the dialog from the context menu and refreshes after save", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /ModDetailDialog/);
  assert.match(source, /detailDialogModId/);
  assert.match(source, /case "info-settings":/);
  assert.match(source, /case "edit-files":/);
  assert.match(source, /refreshModLibrary/);
  assert.match(source, /onSaved=\{refreshModLibrary\}/);
  assert.doesNotMatch(source, /Context Menu Action:/);
});

test("mod detail styles define a floating modal, category chips, and responsive layout", () => {
  const css = readProjectFile("src/features/mods/ModDetailDialog.css");

  assert.match(css, /\.mod-detail-dialog__backdrop\s*{[\s\S]*?position:\s*fixed;/);
  assert.match(css, /\.mod-detail-dialog__panel\s*{[\s\S]*?box-shadow:/);
  assert.match(css, /\.mod-detail-dialog__category-grid\s*{/);
  assert.match(css, /\.mod-detail-dialog__category-chip\s*{/);
  assert.match(css, /@media\s*\(max-width:\s*760px\)/);
});

test("mod detail type mirrors backend metadata DTO", () => {
  const source = readProjectFile("src/features/mods/modLibraryTypes.ts");

  assert.match(source, /metadata:\s*ModPackageMetadata/);
  assert.match(source, /export type ModPackageMetadata/);
  assert.match(source, /author\?:\s*string/);
  assert.match(source, /version\?:\s*string/);
  assert.match(source, /dependencies:\s*string\[]/);
});
