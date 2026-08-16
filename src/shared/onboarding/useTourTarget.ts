import { useLayoutEffect, useState } from "react";
import { expandAndClampRect, rectsEqual, type TourRect } from "./tourGeometry";
import { resolvePreferredTourTarget } from "./tourTarget";
import type { TourAnchorId } from "./tourTypes";

type TourTargetState = {
  requestKey: string | null;
  anchor: TourAnchorId | null;
  element: HTMLElement | null;
  rect: TourRect | null;
  interactionRect: TourRect | null;
};

const EMPTY_TARGET: TourTargetState = {
  requestKey: null,
  anchor: null,
  element: null,
  rect: null,
  interactionRect: null,
};

export function useTourTarget(
  primaryAnchor: TourAnchorId | undefined,
  fallbackAnchor: TourAnchorId | undefined,
  padding: number,
) {
  const [state, setState] = useState<TourTargetState>(EMPTY_TARGET);
  const requestKey = createRequestKey(primaryAnchor, fallbackAnchor);

  useLayoutEffect(() => {
    if (!primaryAnchor && !fallbackAnchor) {
      setState(EMPTY_TARGET);
      return undefined;
    }

    let frameId: number | null = null;
    let resizeObserver: ResizeObserver | null = null;
    let currentTarget: HTMLElement | null = null;

    const measure = () => {
      frameId = null;
      const resolvedTarget = resolvePreferredTourTarget(primaryAnchor, fallbackAnchor);
      const target = resolvedTarget?.element ?? null;

      if (target !== currentTarget) {
        resizeObserver?.disconnect();
        currentTarget = target;
        if (target && typeof ResizeObserver !== "undefined") {
          resizeObserver = new ResizeObserver(scheduleMeasure);
          resizeObserver.observe(target);
          resizeObserver.observe(document.documentElement);
        }
      }

      if (!target) {
        setState((previous) => previous.requestKey === requestKey && previous.element === null
          ? previous
          : { requestKey, anchor: null, element: null, rect: null, interactionRect: null });
        return;
      }

      const targetRect = target.getBoundingClientRect();
      const rect = expandAndClampRect(
        targetRect,
        padding,
        window.innerWidth,
        window.innerHeight,
      );
      const interactionRect = expandAndClampRect(
        targetRect,
        0,
        window.innerWidth,
        window.innerHeight,
      );

      setState((previous) =>
        previous.element === target
          && rectsEqual(previous.rect, rect)
          && rectsEqual(previous.interactionRect, interactionRect)
          ? previous
          : {
              requestKey,
              anchor: resolvedTarget?.anchor ?? null,
              element: target,
              rect,
              interactionRect,
            },
      );
    };

    function scheduleMeasure() {
      if (frameId !== null) return;
      frameId = window.requestAnimationFrame(measure);
    }

    const initialTarget = resolvePreferredTourTarget(primaryAnchor, fallbackAnchor)?.element ?? null;
    if (initialTarget) {
      const rect = initialTarget.getBoundingClientRect();
      const isOutsideSafeViewport = rect.top < 12
        || rect.left < 12
        || rect.bottom > window.innerHeight - 12
        || rect.right > window.innerWidth - 12;
      if (isOutsideSafeViewport) {
        initialTarget.scrollIntoView({
          behavior: prefersReducedMotion() ? "auto" : "smooth",
          block: "center",
          inline: "nearest",
        });
      }
    }

    const mutationObserver = new MutationObserver(scheduleMeasure);
    mutationObserver.observe(document.body, {
      attributes: true,
      childList: true,
      subtree: true,
      attributeFilter: ["aria-hidden", "class", "hidden", "inert"],
    });

    window.addEventListener("resize", scheduleMeasure);
    window.visualViewport?.addEventListener("resize", scheduleMeasure);
    window.visualViewport?.addEventListener("scroll", scheduleMeasure);
    document.addEventListener("scroll", scheduleMeasure, true);
    scheduleMeasure();

    return () => {
      if (frameId !== null) window.cancelAnimationFrame(frameId);
      resizeObserver?.disconnect();
      mutationObserver.disconnect();
      window.removeEventListener("resize", scheduleMeasure);
      window.visualViewport?.removeEventListener("resize", scheduleMeasure);
      window.visualViewport?.removeEventListener("scroll", scheduleMeasure);
      document.removeEventListener("scroll", scheduleMeasure, true);
    };
  }, [fallbackAnchor, padding, primaryAnchor, requestKey]);

  return state.requestKey === requestKey ? state : EMPTY_TARGET;
}

function createRequestKey(
  primaryAnchor: TourAnchorId | undefined,
  fallbackAnchor: TourAnchorId | undefined,
) {
  if (!primaryAnchor && !fallbackAnchor) return null;
  return `${primaryAnchor ?? ""}\u0000${fallbackAnchor ?? ""}`;
}

function prefersReducedMotion() {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}
