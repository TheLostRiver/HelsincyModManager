export const MIN_SCALE = 0.25;
export const MAX_SCALE = 4;
export const SCALE_STEP = 1.25;

export type ViewState = { scale: number; x: number; y: number };

export const INITIAL_VIEW: ViewState = { scale: 1, x: 0, y: 0 };

export function clampScale(value: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, value));
}

/**
 * Keeps the image from being panned completely out of the viewport. The
 * draggable range grows with the zoom level and is zero at 100%, so an image
 * that already fits can never be dragged away.
 */
export function clampOffset(
  next: ViewState,
  viewport: { width: number; height: number },
): ViewState {
  const maxX = Math.max(0, (viewport.width * (next.scale - 1)) / 2);
  const maxY = Math.max(0, (viewport.height * (next.scale - 1)) / 2);
  return {
    scale: next.scale,
    x: Math.min(maxX, Math.max(-maxX, next.x)),
    y: Math.min(maxY, Math.max(-maxY, next.y)),
  };
}

export function normalizeView(
  next: ViewState,
  viewport: { width: number; height: number },
): ViewState {
  return clampOffset({ ...next, scale: clampScale(next.scale) }, viewport);
}

export function isDefaultView(view: ViewState): boolean {
  return view.scale === 1 && view.x === 0 && view.y === 0;
}
