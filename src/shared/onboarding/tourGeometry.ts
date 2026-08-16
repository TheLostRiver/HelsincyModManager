export type TourRect = {
  top: number;
  right: number;
  bottom: number;
  left: number;
  width: number;
  height: number;
};

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
  return rect.width > viewportWidth * 0.72 && rect.height > viewportHeight * 0.55;
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}
