import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  INITIAL_VIEW,
  MAX_SCALE,
  MIN_SCALE,
  SCALE_STEP,
  clampOffset,
  clampScale,
  isDefaultView,
  normalizeView,
} from "./previewImageZoom.ts";

const VIEWPORT = { width: 600, height: 400 };

test("clampScale keeps the zoom level inside the supported range", () => {
  assert.equal(clampScale(0.01), MIN_SCALE);
  assert.equal(clampScale(99), MAX_SCALE);
  assert.equal(clampScale(1), 1);
});

test("clampScale boundaries allow zooming out below and in above 100%", () => {
  assert.ok(MIN_SCALE < 1, "must be able to zoom out below 100%");
  assert.ok(MAX_SCALE > 1, "must be able to zoom in above 100%");
  assert.ok(SCALE_STEP > 1);
});

test("clampOffset forbids panning while the image already fits", () => {
  const view = clampOffset({ scale: 1, x: 400, y: 400 }, VIEWPORT);
  assert.deepEqual(view, { scale: 1, x: 0, y: 0 });
});

test("clampOffset allows panning up to half the overflow", () => {
  const inside = clampOffset({ scale: 2, x: 150, y: 100 }, VIEWPORT);
  assert.deepEqual(inside, { scale: 2, x: 150, y: 100 });

  const outside = clampOffset({ scale: 2, x: 999, y: -999 }, VIEWPORT);
  assert.equal(outside.x, 300);
  assert.equal(outside.y, -200);
});

test("clampOffset keeps the sign of the requested pan direction", () => {
  const left = clampOffset({ scale: 3, x: -999, y: 0 }, VIEWPORT);
  assert.equal(left.x, -600);
  const right = clampOffset({ scale: 3, x: 999, y: 0 }, VIEWPORT);
  assert.equal(right.x, 600);
});

test("normalizeView clamps scale and offset in one step", () => {
  const view = normalizeView({ scale: 99, x: 9999, y: -9999 }, VIEWPORT);
  assert.equal(view.scale, MAX_SCALE);
  assert.equal(view.x, (VIEWPORT.width * (MAX_SCALE - 1)) / 2);
  assert.equal(view.y, -((VIEWPORT.height * (MAX_SCALE - 1)) / 2));
});

test("isDefaultView detects an untouched view only", () => {
  assert.equal(isDefaultView(INITIAL_VIEW), true);
  assert.equal(isDefaultView({ scale: 2, x: 0, y: 0 }), false);
  assert.equal(isDefaultView({ scale: 1, x: 1, y: 0 }), false);
  assert.equal(isDefaultView({ scale: 1, x: 0, y: -1 }), false);
});

test("context menu renders the preview entry only when it is provided", () => {
  const source = readFileSync(new URL("./ModContextMenu.tsx", import.meta.url), "utf8");
  assert.match(source, /previewAction \? \(/);
  assert.match(source, /handleItemClick\("view-preview"\)/);
});

test("library page offers the entry only for mods that have a preview image", () => {
  const source = readFileSync(new URL("./ModLibraryPage.tsx", import.meta.url), "utf8");
  assert.match(source, /item\.previewImage\?\.kind !== "thumbnail"/);
  assert.match(source, /case "view-preview":/);
  assert.match(source, /previewAction=\{contextMenuPreviewAction \?\? undefined\}/);
});

test("preview dialog falls back to the card thumbnail", () => {
  const source = readFileSync(new URL("./PreviewImageDialog.tsx", import.meta.url), "utf8");
  assert.match(source, /detailUrl \?\? fallbackThumbnailUrl/);
  assert.match(source, /onDoubleClick/);
});
