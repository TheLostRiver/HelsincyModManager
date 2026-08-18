import type { TourAnchorId } from "./tourTypes";

export type ResolvedTourTarget = {
  anchor: TourAnchorId;
  element: HTMLElement;
};

export function getTourAnchorCandidates(
  primaryAnchor: TourAnchorId | undefined,
  fallbackAnchor: TourAnchorId | undefined,
) {
  return [primaryAnchor, fallbackAnchor].filter(
    (anchor, index, anchors): anchor is TourAnchorId =>
      Boolean(anchor) && anchors.indexOf(anchor) === index,
  );
}

export function resolvePreferredTourTarget(
  primaryAnchor: TourAnchorId | undefined,
  fallbackAnchor: TourAnchorId | undefined,
): ResolvedTourTarget | null {
  for (const anchor of getTourAnchorCandidates(primaryAnchor, fallbackAnchor)) {
    const element = resolveTourTarget(anchor);
    if (element) return { anchor, element };
  }
  return null;
}

export function resolveTourTarget(anchor: TourAnchorId): HTMLElement | null {
  const candidates = Array.from(
    document.querySelectorAll<HTMLElement>(`[data-tour-id="${anchor}"]`),
  ).filter(isUsableTourTarget);

  return candidates.length === 1 ? candidates[0] : null;
}

export function isUsableTourTarget(element: HTMLElement) {
  if (!element.isConnected || element.closest('[inert], [aria-hidden="true"]')) return false;
  if (element.closest(".route-transition__layer.is-exiting")) return false;
  if (element.closest('[aria-disabled="true"], fieldset:disabled')) return false;
  if (element.hidden) return false;
  if ("disabled" in element && element.disabled === true) return false;

  let current: HTMLElement | null = element;
  while (current) {
    const style = getComputedStyle(current);
    if (
      style.display === "none"
      || style.visibility === "hidden"
      || style.visibility === "collapse"
      || Number.parseFloat(style.opacity) === 0
    ) {
      return false;
    }
    current = current.parentElement;
  }

  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}
