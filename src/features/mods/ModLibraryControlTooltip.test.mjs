import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(fileName) {
  return readFileSync(new URL(`./${fileName}`, import.meta.url), "utf8");
}

test("control tooltip provides an optional accessible description and Escape dismissal", () => {
  const source = readSource("ModLibraryControlTooltip.tsx");

  assert.match(source, /const descriptionId = content && describeControl \? generatedId : undefined/);
  assert.match(source, /role=\{describeControl \? "tooltip" : undefined\}/);
  assert.match(source, /aria-hidden=\{describeControl \? undefined : true\}/);
  assert.match(source, /!content \|\| event\.key !== "Escape"/);
  assert.match(source, /setDismissed\(true\)/);
  assert.match(source, /onPointerLeave=\{\(\) => setDismissed\(false\)\}/);
  assert.match(source, /onFocusCapture=\{\(\) => setDismissed\(false\)\}/);
});

test("control tooltip keeps one wrapper while optional content only toggles the bubble", () => {
  const source = readSource("ModLibraryControlTooltip.tsx");
  const wrapperIndex = source.indexOf('className="mod-library-control-tooltip"');
  const controlIndex = source.indexOf("{children(descriptionId)}", wrapperIndex);
  const conditionalBubbleIndex = source.indexOf("{content ? (", controlIndex);
  const bubbleIndex = source.indexOf('className="mod-library-control-tooltip__bubble"', conditionalBubbleIndex);

  assert.doesNotMatch(source, /if \(!content\)\s*\{\s*return children\(undefined\);\s*\}/);
  assert.ok(wrapperIndex >= 0);
  assert.ok(controlIndex > wrapperIndex);
  assert.ok(conditionalBubbleIndex > controlIndex);
  assert.ok(bubbleIndex > conditionalBubbleIndex);
});

test("control tooltip is token-based, hoverable, focusable, compact, and reduced-motion aware", () => {
  const css = readSource("ModLibraryControlTooltip.css");

  assert.match(css, /\.mod-library-control-tooltip:hover\s*>/);
  assert.match(css, /\.mod-library-control-tooltip:focus-within\s*>/);
  assert.match(css, /pointer-events:\s*auto/);
  assert.match(css, /data-tooltip-dismissed="true"/);
  assert.match(css, /max-inline-size:\s*min\(260px,\s*calc\(100vw - 32px\)\)/);
  assert.match(css, /border-radius:\s*6px/);
  assert.match(css, /var\(--color-text\)/);
  assert.match(css, /var\(--color-surface\)/);
  assert.match(css, /var\(--shadow-panel\)/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  assert.doesNotMatch(css, /#[0-9a-f]{3,8}\b/i);
});

test("toolbar icon controls use custom tooltips with explicit accessible names", () => {
  const source = readSource("LibraryToolbar.tsx");
  const css = readSource("ModLibraryControlTooltip.css");

  assert.match(source, /<ModLibraryControlTooltip content=\{labelToggleTitle\} describeControl=\{false\}>/);
  assert.match(source, /aria-label=\{labelToggleTitle\}/);

  for (const label of ["经典简约视图", "增强网格视图", "紧凑列表视图", "机能数据面板视图"]) {
    assert.match(source, new RegExp(`<ModLibraryControlTooltip content="${label}" describeControl=\\{false\\}>`));
    assert.match(source, new RegExp(`aria-label="${label}"`));
  }

  assert.doesNotMatch(source, /\btitle=/);
  assert.match(css, /\.library-toolbar__display-controls \.library-view-toggles\s*\{[\s\S]*?overflow:\s*visible;/);
  assert.match(css, /\.library-view-toggles\s*>\s*\.mod-library-control-tooltip\s*\{[\s\S]*?inline-size:\s*var\(--view-toggle-size\);/);
  assert.match(css, /\.library-toolbar__display-controls[\s\S]*?inset-block-start:\s*calc\(100% \+ 8px\);/);
  assert.match(css, /\.library-view-toggles[\s\S]*?> \.mod-library-control-tooltip:last-child[\s\S]*?inset-inline-end:\s*0;/);
});
