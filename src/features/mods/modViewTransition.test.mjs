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

function getRuleBodies(css, selector) {
  const bodies = [];
  let searchFrom = 0;

  while (searchFrom < css.length) {
    const start = css.indexOf(`${selector} {`, searchFrom);
    if (start < 0) {
      break;
    }

    const openBraceIndex = css.indexOf("{", start);
    const closeBraceIndex = css.indexOf("}", openBraceIndex);
    assert.ok(openBraceIndex >= 0 && closeBraceIndex > openBraceIndex, `invalid CSS rule: ${selector}`);
    bodies.push(css.slice(openBraceIndex + 1, closeBraceIndex));
    searchFrom = closeBraceIndex + 1;
  }

  assert.ok(bodies.length > 0, `missing CSS rule: ${selector}`);
  return bodies;
}

test("view mode toggle owns a sliding selected-state indicator", () => {
  const toolbar = readProjectFile("src/features/mods/LibraryToolbar.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(toolbar, /const viewModeIndex/);
  assert.match(toolbar, /library-view-toggle-indicator/);
  assert.match(toolbar, /"--view-toggle-index": viewModeIndex/);

  const groupBody = getRuleBody(css, ".library-view-toggles");
  assert.match(groupBody, /position:\s*relative;/);
  assert.match(groupBody, /--view-toggle-size:\s*28px;/);

  const indicatorBody = getRuleBody(css, ".library-view-toggle-indicator");
  assert.match(indicatorBody, /position:\s*absolute;/);
  assert.match(indicatorBody, /transform:\s*translateX\(calc\(var\(--view-toggle-index\)\s*\*\s*\(var\(--view-toggle-size\)\s*\+\s*var\(--view-toggle-gap\)\)\)\);/);
  assert.match(indicatorBody, /transition:\s*transform/);
  assert.match(indicatorBody, /pointer-events:\s*none;/);
});

test("mod library switches views through a visible two-phase transition", () => {
  const page = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const card = readProjectFile("src/features/mods/ModPosterCard.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(page, /useModViewTransition/);
  assert.match(page, /viewTransitionPhase/);
  assert.match(page, /viewTransitionVariant/);
  assert.match(page, /setViewTransitionPhase\("out"\)/);
  assert.match(page, /setViewTransitionPhase\("in"\)/);
  assert.match(page, /data-view-transition={viewTransitionPhase}/);
  assert.match(page, /data-view-transition-variant={viewTransitionVariant}/);
  assert.match(page, /onViewModeChange={handleViewModeChange}/);
  assert.doesNotMatch(page, /\.animate\(\s*\[/);
  assert.match(card, /className={`mod-card\$\{selected \? " is-selected" : ""\}`}/);
  assert.doesNotMatch(card, /mod-card anim-stagger-item/);

  assert.match(
    css,
    /\.mod-grid\[data-view-transition="out"\]\s+\.mod-card,\s*\.mod-grid\[data-view-transition="in"\]\s+\.mod-card\s*{[\s\S]*?pointer-events:\s*none;/,
  );
  assert.match(
    css,
    /\.mod-grid\[data-view-transition="out"\]\s+\.mod-card,\s*\.mod-grid\[data-view-transition="in"\]\s+\.mod-card\s*{[\s\S]*?will-change:\s*transform,\s*opacity,\s*filter;/,
  );

  assert.match(
    css,
    /\.mod-grid\[data-view-transition="in"\]\s+\.mod-card\s*{[\s\S]*?animation-delay:\s*calc\(min\(var\(--stagger-idx\),\s*12\)\s*\*\s*18ms\);/,
  );

  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*{[\s\S]*?\.mod-grid\[data-view-transition="out"\]\s+\.mod-card/);
});

test("reduced-motion view switches clear pending transition timers first", () => {
  const page = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(
    page,
    /if \(outTimeoutRef\.current !== null\) \{[\s\S]*?window\.clearTimeout\(outTimeoutRef\.current\);[\s\S]*?outTimeoutRef\.current = null;[\s\S]*?\}[\s\S]*?if \(inTimeoutRef\.current !== null\) \{[\s\S]*?window\.clearTimeout\(inTimeoutRef\.current\);[\s\S]*?inTimeoutRef\.current = null;[\s\S]*?\}[\s\S]*?if \(prefersReducedMotion\(\)\)/,
  );
});

test("each target view maps to a distinct list transition variant", () => {
  const page = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(page, /type ViewTransitionVariant = "morph" \| "wave" \| "flip3d" \| "blur";/);
  assert.match(page, /classic:\s*"morph"/);
  assert.match(page, /grid:\s*"wave"/);
  assert.match(page, /list:\s*"flip3d"/);
  assert.match(page, /tech:\s*"blur"/);
  assert.match(page, /setViewTransitionVariant\(viewTransitionVariantByMode\[nextViewMode\]\)/);

  const variantAnimations = [
    ["morph", "mod-view-card-enter-morph"],
    ["wave", "mod-view-card-enter-wave"],
    ["flip3d", "mod-view-card-enter-flip3d"],
    ["blur", "mod-view-card-enter-blur"],
  ];

  for (const [variant, animationName] of variantAnimations) {
    const selector = `.mod-grid[data-view-transition="in"][data-view-transition-variant="${variant}"] .mod-card`;
    const bodies = getRuleBodies(css, selector);
    assert.ok(
      bodies.some((body) => new RegExp(`animation:\\s*${animationName}`).test(body)),
      `${selector} should define ${animationName}`,
    );
    assert.match(css, new RegExp(`@keyframes\\s+${animationName}`));
  }

  assert.match(
    css,
    /\.mod-grid\[data-view-transition="out"\]\[data-view-transition-variant="flip3d"\],\s*\.mod-grid\[data-view-transition="in"\]\[data-view-transition-variant="flip3d"\]\s*{[\s\S]*?perspective:\s*1200px;/,
  );
  assert.match(
    css,
    /\.mod-grid\[data-view-transition="out"\]\[data-view-transition-variant="flip3d"\]\s+\.mod-card,\s*\.mod-grid\[data-view-transition="in"\]\[data-view-transition-variant="flip3d"\]\s+\.mod-card\s*{[\s\S]*?backface-visibility:\s*hidden;[\s\S]*?transform-style:\s*preserve-3d;/,
  );
  assert.doesNotMatch(css, /\.mod-grid\[data-view-transition-variant="flip3d"\]\s*{/);
  assert.doesNotMatch(css, /\.mod-grid\[data-view-transition-variant="flip3d"\]\s+\.mod-card\s*{/);
});
