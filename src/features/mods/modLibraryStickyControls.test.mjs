import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

function getRuleBody(css, selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `missing CSS rule: ${selector}`);

  const openBraceIndex = css.indexOf("{", start);
  const closeBraceIndex = css.indexOf("}", openBraceIndex);
  assert.ok(openBraceIndex >= 0 && closeBraceIndex > openBraceIndex, `invalid CSS rule: ${selector}`);
  return css.slice(openBraceIndex + 1, closeBraceIndex);
}

test("ModLibraryPage groups toolbar and quick actions into one sticky controls area", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /className="mod-library__sticky-controls anim-stagger-item"/);
  assert.match(source, /className="mod-library__toolbar-slot"/);
  assert.match(source, /className="mod-library__actions-slot"/);
  assert.match(source, /<LibraryToolbar[\s\S]*?query={query}[\s\S]*?activeFilter={activeFilter}/);
  assert.match(source, /<CompactActionPanel selectedCount={selectedCount} onAction={handleAction} \/>/);

  const toolbarIndex = source.indexOf("mod-library__toolbar-slot");
  const actionsIndex = source.indexOf("mod-library__actions-slot");
  const gridIndex = source.indexOf("mod-library__content");

  assert.ok(toolbarIndex > -1, "toolbar slot should exist");
  assert.ok(actionsIndex > -1, "actions slot should exist");
  assert.ok(gridIndex > -1, "content area should exist");
  assert.ok(toolbarIndex < gridIndex, "toolbar slot should render before content");
  assert.ok(actionsIndex < gridIndex, "actions slot should render before content");
});

test("sticky controls use app-surface friendly sticky positioning instead of fixed positioning", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-sticky-top:\s*var\(--layout-page-padding\);/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?display:\s*grid;/);
  assert.match(
    css,
    /\.mod-library__sticky-controls\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\)\s+var\(--layout-mod-action-panel-width\);/,
  );
  assert.match(css, /\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?position:\s*sticky;/);
  assert.match(
    css,
    /\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?top:\s*var\(--mod-library-sticky-top\);/,
  );
  assert.doesNotMatch(css, /\.library-toolbar[\s\S]*?position:\s*fixed;/);
  assert.doesNotMatch(css, /\.compact-panel[\s\S]*?position:\s*fixed;/);
});

test("narrow layouts collapse controls into a single sticky top area and keep quick actions horizontally scrollable", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.mod-library__sticky-controls\s*{[\s\S]*?position:\s*sticky;[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?position:\s*static;/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/,
  );
});

test("quick action panel no longer owns sticky positioning directly", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  const compactPanelBody = getRuleBody(css, ".compact-panel");

  assert.doesNotMatch(compactPanelBody, /position:\s*sticky;/);
  assert.match(compactPanelBody, /min-width:\s*0;/);
});
