type TrappedFocusIndexInput = {
  currentIndex: number;
  focusableCount: number;
  backwards: boolean;
};

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "textarea:not([disabled])",
  "select:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export function getTrappedFocusIndex({ currentIndex, focusableCount, backwards }: TrappedFocusIndexInput) {
  if (focusableCount <= 0) {
    return -1;
  }

  if (currentIndex < 0) {
    return backwards ? focusableCount - 1 : 0;
  }

  if (backwards && currentIndex === 0) {
    return focusableCount - 1;
  }

  if (!backwards && currentIndex === focusableCount - 1) {
    return 0;
  }

  return null;
}

export function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.tabIndex >= 0 && isElementRendered(element),
  );
}

export function isTopmostModalSurface(container: HTMLElement): boolean {
  const visibleModals = Array.from(document.querySelectorAll<HTMLElement>('[aria-modal="true"]')).filter(
    isElementRendered,
  );
  let topmost: HTMLElement | null = null;
  let topmostLayer = Number.NEGATIVE_INFINITY;

  for (const modal of visibleModals) {
    const layer = getModalLayer(modal);
    if (layer >= topmostLayer) {
      topmost = modal;
      topmostLayer = layer;
    }
  }

  return topmost === container;
}

function isElementRendered(element: HTMLElement): boolean {
  if (element.getClientRects().length === 0) {
    return false;
  }

  let current: HTMLElement | null = element;
  while (current) {
    if (
      current.hasAttribute("hidden")
      || current.hasAttribute("inert")
      || current.getAttribute("aria-hidden") === "true"
    ) {
      return false;
    }

    const style = getComputedStyle(current);
    if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") {
      return false;
    }

    current = current.parentElement;
  }

  return true;
}

function getModalLayer(modal: HTMLElement): number {
  let current: HTMLElement | null = modal;

  while (current) {
    const zIndex = Number.parseInt(getComputedStyle(current).zIndex, 10);
    if (Number.isFinite(zIndex)) {
      return zIndex;
    }
    current = current.parentElement;
  }

  return 0;
}
