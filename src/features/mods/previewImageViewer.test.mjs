import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  INITIAL_VIEW,
  MAX_SCALE,
  MIN_SCALE,
  SCALE_STEP,
  advanceDrag,
  clampOffset,
  clampScale,
  fittedImageSize,
  isDefaultView,
  normalizeView,
} from "./previewImageZoom.ts";

const VIEWPORT = { width: 600, height: 400 };
/** Same 3:2 ratio as the viewport, so the fitted box fills it exactly. */
const FILLING_IMAGE = { width: 900, height: 600 };
/** Tall portrait: letterboxed left and right inside the viewport. */
const PORTRAIT_IMAGE = { width: 300, height: 900 };

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
  const view = clampOffset({ scale: 1, x: 400, y: 400 }, VIEWPORT, FILLING_IMAGE);
  assert.deepEqual(view, { scale: 1, x: 0, y: 0 });
});

test("clampOffset allows panning up to half the overflow", () => {
  const inside = clampOffset({ scale: 2, x: 150, y: 100 }, VIEWPORT, FILLING_IMAGE);
  assert.deepEqual(inside, { scale: 2, x: 150, y: 100 });

  const outside = clampOffset({ scale: 2, x: 999, y: -999 }, VIEWPORT, FILLING_IMAGE);
  assert.equal(outside.x, 300);
  assert.equal(outside.y, -200);
});

test("clampOffset keeps the sign of the requested pan direction", () => {
  const left = clampOffset({ scale: 3, x: -999, y: 0 }, VIEWPORT, FILLING_IMAGE);
  assert.equal(left.x, -600);
  const right = clampOffset({ scale: 3, x: 999, y: 0 }, VIEWPORT, FILLING_IMAGE);
  assert.equal(right.x, 600);
});

test("normalizeView clamps scale and offset in one step", () => {
  const view = normalizeView({ scale: 99, x: 9999, y: -9999 }, VIEWPORT, FILLING_IMAGE);
  assert.equal(view.scale, MAX_SCALE);
  assert.equal(view.x, (VIEWPORT.width * (MAX_SCALE - 1)) / 2);
  assert.equal(view.y, -((VIEWPORT.height * (MAX_SCALE - 1)) / 2));
});

test("fittedImageSize preserves the aspect ratio inside the viewport", () => {
  // Height-limited portrait: 300x900 into 600x400 -> 133.33x400.
  const fitted = fittedImageSize(PORTRAIT_IMAGE, VIEWPORT);
  assert.ok(Math.abs(fitted.height - VIEWPORT.height) < 1e-9);
  assert.ok(Math.abs(fitted.width - (VIEWPORT.height * 300) / 900) < 1e-9);
  assert.ok(Math.abs(fitted.width / fitted.height - 300 / 900) < 1e-9);

  assert.deepEqual(fittedImageSize({ width: 0, height: 0 }, VIEWPORT), {
    width: 0,
    height: 0,
  });
});

test("a letterboxed axis cannot be panned until the image actually overflows", () => {
  // The painted width is ~133px inside a 600px viewport. It only starts
  // overflowing horizontally once the zoom passes ~4.5x, so at 2x there is
  // nothing to pan sideways — dragging must not move the image off-screen.
  const fitted = fittedImageSize(PORTRAIT_IMAGE, VIEWPORT);
  assert.ok(fitted.width < VIEWPORT.width, "fixture must be letterboxed");

  const sideways = clampOffset({ scale: 2, x: 9999, y: 0 }, VIEWPORT, PORTRAIT_IMAGE);
  assert.equal(sideways.x, 0, "no horizontal overflow means no horizontal pan");

  // Vertically the image fills the viewport, so zooming in does give room.
  const vertical = clampOffset({ scale: 2, x: 0, y: 9999 }, VIEWPORT, PORTRAIT_IMAGE);
  assert.equal(vertical.y, (fitted.height * 2 - VIEWPORT.height) / 2);
});

test("the image can never be dragged completely out of the viewport", () => {
  // Invariant: at the extreme offset the image still covers as much of the
  // viewport as it possibly can — either the whole viewport (when the painted
  // image is larger) or the whole painted image (when it is smaller).
  // The old viewport-based range violated this on letterboxed axes: dragging
  // right slid the *left* edge off-screen, leaving visible blank space.
  const overlap = (viewportSpan, painted, offset) => {
    const left = (viewportSpan - painted) / 2 + offset;
    const right = (viewportSpan + painted) / 2 + offset;
    return Math.min(viewportSpan, right) - Math.max(0, left);
  };
  const coveredAtMost = (viewportSpan, painted) => Math.min(viewportSpan, painted);

  const images = [FILLING_IMAGE, PORTRAIT_IMAGE, { width: 1800, height: 400 }];
  for (const image of images) {
    for (const scale of [0.25, 1, 1.25, 2, 4]) {
      const fitted = fittedImageSize(image, VIEWPORT);
      const paintedWidth = fitted.width * scale;
      const paintedHeight = fitted.height * scale;
      const label = `${image.width}x${image.height} @${scale}`;

      for (const [dx, dy] of [[99999, 99999], [-99999, -99999], [99999, -99999]]) {
        const pushed = clampOffset({ scale, x: dx, y: dy }, VIEWPORT, image);
        const coveredX = overlap(VIEWPORT.width, paintedWidth, pushed.x);
        const coveredY = overlap(VIEWPORT.height, paintedHeight, pushed.y);
        assert.ok(
          coveredX >= coveredAtMost(VIEWPORT.width, paintedWidth) - 1e-9,
          `${label} x:${dx}: covers ${coveredX} of ${coveredAtMost(VIEWPORT.width, paintedWidth)}`,
        );
        assert.ok(
          coveredY >= coveredAtMost(VIEWPORT.height, paintedHeight) - 1e-9,
          `${label} y:${dy}: covers ${coveredY} of ${coveredAtMost(VIEWPORT.height, paintedHeight)}`,
        );
      }
    }
  }
});

test("drag responds immediately after being pulled back from the limit", () => {
  // Regression: the anchor must be re-set to the *committed* offset on every
  // sample. Anchoring on the raw pointer instead accumulates the whole
  // overshoot, so dragging far past the edge creates a dead zone the pointer
  // has to travel back through before the image moves at all — which reads as
  // "it got stuck" once you have dragged into the blank area.
  const scale = 2;
  const maxX = (VIEWPORT.width * (scale - 1)) / 2; // 300 for a filling image

  let anchor = { px: 0, py: 0, ox: 0, oy: 0 };
  const dragTo = (x) => {
    const stepped = advanceDrag(anchor, { x, y: 0 }, scale, VIEWPORT, FILLING_IMAGE);
    anchor = stepped.anchor;
    return stepped.view;
  };

  const atLimit = dragTo(maxX + 700);
  assert.equal(atLimit.x, maxX, "the overshoot is clamped away");

  // One pixel back must move the image one pixel. No dead zone.
  assert.equal(dragTo(maxX + 700 - 1).x, maxX - 1);
  assert.equal(dragTo(maxX + 700 - 2).x, maxX - 2);
  // ...and it keeps tracking all the way back through the origin.
  assert.equal(dragTo(maxX + 700 - (maxX + 40)).x, -40);
});

test("a drag past the limit never accumulates overshoot in either direction", () => {
  // Invariant form of the dead-zone regression: for every pointer sample the
  // committed offset must equal "previous committed offset + pointer delta",
  // clamped. Anchoring on raw pointer movement breaks this after any overshoot.
  const scale = 2;
  const maxX = (VIEWPORT.width * (scale - 1)) / 2;
  const walk = [500, 1200, -900, 40, 4000, 3990, -20, 15];

  let anchor = { px: 0, py: 0, ox: 0, oy: 0 };
  let pointer = 0;
  let offset = 0;
  for (const x of walk) {
    const stepped = advanceDrag(anchor, { x, y: 0 }, scale, VIEWPORT, FILLING_IMAGE);
    const expected = Math.min(maxX, Math.max(-maxX, offset + (x - pointer)));

    assert.ok(
      Math.abs(stepped.view.x - expected) < 1e-9,
      `pointer ${pointer} -> ${x}: expected ${expected}, got ${stepped.view.x}`,
    );
    assert.ok(stepped.view.x >= -maxX - 1e-9 && stepped.view.x <= maxX + 1e-9);
    assert.equal(stepped.anchor.px, x, "the pointer origin follows the sample");
    assert.equal(stepped.anchor.ox, stepped.view.x, "the offset origin is the committed one");

    anchor = stepped.anchor;
    pointer = x;
    offset = stepped.view.x;
  }

  // Overshoot is discarded, not banked: after being pinned at +maxX and then
  // slammed to -maxX, one pixel back moves exactly one pixel.
  assert.equal(offset, -265);
  const back = advanceDrag(anchor, { x: 16, y: 0 }, scale, VIEWPORT, FILLING_IMAGE);
  assert.equal(back.view.x, -264);
});

test("dragging a letterboxed axis sideways cannot create a hidden dead zone", () => {
  // Portrait at 2x has zero horizontal range. Pushing right and then pulling
  // back must stay pinned at 0 rather than banking movement that reappears later.
  let anchor = { px: 0, py: 0, ox: 0, oy: 0 };
  const dragTo = (x) => {
    const stepped = advanceDrag(anchor, { x, y: 0 }, 2, VIEWPORT, PORTRAIT_IMAGE);
    anchor = stepped.anchor;
    return stepped.view;
  };
  assert.equal(dragTo(900).x, 0);
  assert.equal(dragTo(400).x, 0);
  assert.equal(dragTo(-900).x, 0);
  assert.equal(dragTo(0).x, 0);
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
