import { useCallback, useLayoutEffect, useState } from "react";
import { expandAndClampRect, rectsEqual, type TourRect } from "./tourGeometry";
import { resolvePreferredTourTarget } from "./tourTarget";
import type { TourAnchorId } from "./tourTypes";

type TourTargetState = {
  requestKey: string | null;
  anchor: TourAnchorId | null;
  element: HTMLElement | null;
  rect: TourRect | null;
  interactionRect: TourRect | null;
  timedOut: boolean;
};

const EMPTY_TARGET: TourTargetState = {
  requestKey: null,
  anchor: null,
  element: null,
  rect: null,
  interactionRect: null,
  timedOut: false,
};

const TOUR_TARGET_WAIT_MS = 1_800;
const TOUR_TARGET_ANIMATION_POLL_MS = 1_200;

export function useTourTarget(
  primaryAnchor: TourAnchorId | undefined,
  fallbackAnchor: TourAnchorId | undefined,
  padding: number,
) {
  const [state, setState] = useState<TourTargetState>(EMPTY_TARGET);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const requestKey = createRequestKey(primaryAnchor, fallbackAnchor);
  const retry = useCallback(() => setRetryAttempt((attempt) => attempt + 1), []);

  useLayoutEffect(() => {
    if (!primaryAnchor && !fallbackAnchor) {
      setState(EMPTY_TARGET);
      return undefined;
    }

    let frameId: number | null = null;
    let animationPollFrameId: number | null = null;
    let timeoutId: number | null = null;
    let resizeObserver: ResizeObserver | null = null;
    let currentTarget: HTMLElement | null = null;
    const animationPollDeadline = performance.now() + TOUR_TARGET_ANIMATION_POLL_MS;

    const markUnavailableAfterWait = () => {
      if (timeoutId !== null) return;
      timeoutId = window.setTimeout(() => {
        timeoutId = null;
        setState((previous) => previous.requestKey === requestKey && previous.element === null
          ? { ...previous, timedOut: true }
          : previous);
      }, TOUR_TARGET_WAIT_MS);
    };

    setState((previous) => previous.requestKey === requestKey
      ? { ...previous, timedOut: false }
      : {
          requestKey,
          anchor: null,
          element: null,
          rect: null,
          interactionRect: null,
          timedOut: false,
        });

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
        markUnavailableAfterWait();
        setState((previous) => previous.requestKey === requestKey && previous.element === null
          ? previous
          : {
              requestKey,
              anchor: null,
              element: null,
              rect: null,
              interactionRect: null,
              timedOut: false,
            });
        return;
      }

      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
        timeoutId = null;
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
              timedOut: false,
            },
      );
    };

    function scheduleMeasure() {
      if (frameId !== null) return;
      frameId = window.requestAnimationFrame(measure);
    }

    function pollAnimatedTarget(timestamp: number) {
      scheduleMeasure();
      if (timestamp < animationPollDeadline && currentTarget === null) {
        animationPollFrameId = window.requestAnimationFrame(pollAnimatedTarget);
      } else {
        animationPollFrameId = null;
      }
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
    const observationRoot = document.getElementById("root") ?? document.body;
    mutationObserver.observe(observationRoot, {
      attributes: true,
      childList: true,
      subtree: true,
      attributeFilter: ["aria-hidden", "class", "hidden", "inert"],
    });

    const scrollListenerOptions = { capture: true, passive: true } as const;
    window.addEventListener("resize", scheduleMeasure);
    window.visualViewport?.addEventListener("resize", scheduleMeasure);
    window.visualViewport?.addEventListener("scroll", scheduleMeasure);
    document.addEventListener("scroll", scheduleMeasure, scrollListenerOptions);
    scheduleMeasure();
    animationPollFrameId = window.requestAnimationFrame(pollAnimatedTarget);
    markUnavailableAfterWait();

    return () => {
      if (frameId !== null) window.cancelAnimationFrame(frameId);
      if (animationPollFrameId !== null) window.cancelAnimationFrame(animationPollFrameId);
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      resizeObserver?.disconnect();
      mutationObserver.disconnect();
      window.removeEventListener("resize", scheduleMeasure);
      window.visualViewport?.removeEventListener("resize", scheduleMeasure);
      window.visualViewport?.removeEventListener("scroll", scheduleMeasure);
      document.removeEventListener("scroll", scheduleMeasure, scrollListenerOptions);
    };
  }, [fallbackAnchor, padding, primaryAnchor, requestKey, retryAttempt]);

  return {
    ...(state.requestKey === requestKey ? state : EMPTY_TARGET),
    retry,
  };
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
