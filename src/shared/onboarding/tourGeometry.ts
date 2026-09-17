export type TourRect = {
  top: number;
  right: number;
  bottom: number;
  left: number;
  width: number;
  height: number;
};

export const TOUR_PANEL_LAYOUT = {
  width: 440,
  minHeight: 280,
  viewportPadding: 16,
  targetGap: 16,
} as const;

export function expandAndClampRect(
  rect: Pick<DOMRectReadOnly, "top" | "right" | "bottom" | "left">,
  padding: number,
  viewportWidth: number,
  viewportHeight: number,
): TourRect {
  const left = clamp(rect.left - padding, 0, viewportWidth);
  const top = clamp(rect.top - padding, 0, viewportHeight);
  const right = clamp(rect.right + padding, 0, viewportWidth);
  const bottom = clamp(rect.bottom + padding, 0, viewportHeight);

  return {
    top,
    right,
    bottom,
    left,
    width: Math.max(0, right - left),
    height: Math.max(0, bottom - top),
  };
}

export function rectsEqual(left: TourRect | null, right: TourRect, tolerance = 0.25) {
  if (!left) return false;
  return Math.abs(left.top - right.top) <= tolerance
    && Math.abs(left.right - right.right) <= tolerance
    && Math.abs(left.bottom - right.bottom) <= tolerance
    && Math.abs(left.left - right.left) <= tolerance;
}

export function shouldDockTourPanel(
  rect: TourRect | null,
  viewportWidth: number,
  viewportHeight: number,
) {
  if (viewportWidth <= 600) return true;
  if (!rect) return false;

  const { width, minHeight, viewportPadding, targetGap } = TOUR_PANEL_LAYOUT;
  const clearance = viewportPadding + targetGap;
  const fitsAboveOrBelow = viewportWidth - 2 * viewportPadding >= width
    && Math.max(rect.top, viewportHeight - rect.bottom) - clearance >= minHeight;
  const fitsBeside = viewportHeight - 2 * viewportPadding >= minHeight
    && Math.max(rect.left, viewportWidth - rect.right) - clearance >= width;

  return !fitsAboveOrBelow && !fitsBeside;
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}
