export const MIN_SCALE = 0.25;
export const MAX_SCALE = 4;
export const SCALE_STEP = 1.25;

export type ViewState = { scale: number; x: number; y: number };
export type Size = { width: number; height: number };

export const INITIAL_VIEW: ViewState = { scale: 1, x: 0, y: 0 };

export function clampScale(value: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, value));
}

/**
 * The box the `<img>` actually occupies: the largest aspect-preserving box for
 * `image` inside `viewport`. Mirrors the dialog's `max-width/max-height: 100%`
 * plus `object-fit: contain` — keep the two in sync if the CSS changes.
 */
export function fittedImageSize(image: Size, viewport: Size): Size {
  if (
    image.width <= 0
    || image.height <= 0
    || viewport.width <= 0
    || viewport.height <= 0
  ) {
    return { width: 0, height: 0 };
  }
  const fit = Math.min(viewport.width / image.width, viewport.height / image.height);
  return { width: image.width * fit, height: image.height * fit };
}

/**
 * Keeps the image from being panned out of the viewport.
 *
 * The range is driven by the **rendered image size**, not the viewport. Under
 * `object-fit: contain` a letterboxed image is much smaller than the viewport on
 * one axis: a portrait in a wide viewport is height-limited, so its painted width
 * is a fraction of the viewport width. Using the viewport size here let the image
 * be dragged completely off-screen on whichever axis was letterboxed, even though
 * that axis had no overflow to pan.
 *
 * Until the image size is known (before load) the range is zero, so nothing can
 * be dragged out of view early.
 */
export function clampOffset(next: ViewState, viewport: Size, image: Size): ViewState {
  const fitted = fittedImageSize(image, viewport);
  const maxX = Math.max(0, (fitted.width * next.scale - viewport.width) / 2);
  const maxY = Math.max(0, (fitted.height * next.scale - viewport.height) / 2);
  return {
    scale: next.scale,
    // `+ 0` turns the `-0` a clamped axis produces into `+0`, so the committed
    // offset stays comparable with `===` against INITIAL_VIEW.
    x: Math.min(maxX, Math.max(-maxX, next.x)) + 0,
    y: Math.min(maxY, Math.max(-maxY, next.y)) + 0,
  };
}

export function normalizeView(next: ViewState, viewport: Size, image: Size): ViewState {
  return clampOffset({ ...next, scale: clampScale(next.scale) }, viewport, image);
}

export type DragAnchor = { px: number; py: number; ox: number; oy: number };
export type Point = { x: number; y: number };

/**
 * Advances a drag by one pointer sample and returns the committed view together
 * with the re-anchored drag origin.
 *
 * The re-anchor is what prevents a dead zone: the requested offset is clamped,
 * so if the anchor kept accumulating raw pointer movement the image would only
 * start moving again after the pointer had travelled the entire overshoot back.
 * Dragging into the blank area and then pulling back a little must respond
 * immediately.
 */
export function advanceDrag(
  anchor: DragAnchor,
  pointer: Point,
  scale: number,
  viewport: Size,
  image: Size,
): { view: ViewState; anchor: DragAnchor } {
  const view = clampOffset(
    {
      scale,
      x: anchor.ox + (pointer.x - anchor.px),
      y: anchor.oy + (pointer.y - anchor.py),
    },
    viewport,
    image,
  );
  return {
    view,
    anchor: { px: pointer.x, py: pointer.y, ox: view.x, oy: view.y },
  };
}

export function isDefaultView(view: ViewState): boolean {
  return view.scale === 1 && view.x === 0 && view.y === 0;
}
