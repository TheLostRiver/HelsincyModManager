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

test("category page owns a full management layout instead of a tiny inline panel", () => {
  const source = readProjectFile("src/features/categories/CategoryPage.tsx");

  assert.match(source, /className="category-page__header"/);
  assert.match(source, /className="category-page__summary-grid"/);
  assert.match(source, /className="category-workspace"/);
  assert.match(source, /className="category-main-card"/);
  assert.match(source, /className="category-list__header"/);
  assert.match(source, /className="category-state-card is-error"/);
  assert.match(source, /className="category-state-card is-empty"/);
  assert.doesNotMatch(source, /useSidebarMode|sidebarMode/);
});

test("category page CSS fills the route cleanly and stays responsive", () => {
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(css, /\.route-transition__layer\[data-route-id="categories"\]\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/);
  assert.match(getRuleBody(css, ".category-page"), /gap:\s*var\(--layout-content-gap\);/);
  assert.match(getRuleBody(css, ".category-main-card"), /box-shadow:\s*var\(--shadow-soft\);/);
  assert.match(getRuleBody(css, ".category-list__header"), /grid-template-columns:\s*var\(--category-list-columns\);/);
  assert.match(getRuleBody(css, ".category-state-card.is-error"), /border:\s*1px\s+solid/);
  assert.doesNotMatch(getRuleBody(css, ".category-state-card.is-error"), /background:\s*var\(--color-warning-bg\);/);
  assert.match(css, /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*?\.category-workspace\s*{[\s\S]*?grid-template-columns:\s*1fr;/);
});

test("category color editing uses a visual picker instead of hex text entry", () => {
  const source = readProjectFile("src/features/categories/CategoryPage.tsx");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(source, /CATEGORY_COLOR_OPTIONS/);
  assert.match(source, /className="category-color-picker"/);
  assert.match(source, /className="category-color-picker__trigger"/);
  assert.match(source, /aria-expanded=\{open\}/);
  assert.match(source, /className="category-color-popover"/);
  assert.match(source, /className="category-color-palette"/);
  assert.match(source, /className=\{`category-color-swatch-button/);
  assert.match(source, /type="color"/);
  assert.match(source, /className="category-color-clear"/);
  assert.doesNotMatch(source, /placeholder="#3B82F6"/);
  assert.doesNotMatch(source, /category-color-text/);
  assert.match(getRuleBody(css, ".category-color-picker"), /position:\s*relative;/);
  assert.match(getRuleBody(css, ".category-color-popover"), /position:\s*absolute;/);
  assert.match(getRuleBody(css, ".category-color-popover"), /z-index:\s*30;/);
  assert.match(css, /\.category-color-palette\s*{/);
  assert.match(css, /\.category-color-swatch-button\s*{/);
});
