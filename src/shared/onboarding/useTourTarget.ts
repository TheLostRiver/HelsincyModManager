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
/** 判定目标是否贴边的安全内缩，同时也是滚动后期望留出的余量。 */
const TOUR_TARGET_SAFE_INSET = 12;

/**
 * 目标贴近或超出视口边缘时把它滚到中间。
 *
 * 只在需要时滚动：目标已完整落在安全区内就不动，避免每次重新测量都抢走
 * 用户自己的滚动位置。用 block: "center" 而非 "nearest"，因为高亮矩形会被
 * 钳到视口范围内，目标居中才能保证整块都画得出来。
 */
function scrollTargetIntoSafeViewport(target: HTMLElement) {
  const rect = target.getBoundingClientRect();
  const isOutsideSafeViewport = rect.top < TOUR_TARGET_SAFE_INSET
    || rect.left < TOUR_TARGET_SAFE_INSET
    || rect.bottom > window.innerHeight - TOUR_TARGET_SAFE_INSET
    || rect.right > window.innerWidth - TOUR_TARGET_SAFE_INSET;

  if (!isOutsideSafeViewport) return;

  target.scrollIntoView({
    behavior: prefersReducedMotion() ? "auto" : "smooth",
    block: "center",
    inline: "nearest",
  });
}

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
        /*
         * 滚动跟着"元素身份变化"走，而不是在 effect 顶层做一次性检查。
         *
         * 路由层进场动画 route-layer-enter 的 from { opacity: 0 } 在
         * animation-fill-mode: both 下让动画启动前的计算 opacity 就是 0，
         * 而 isUsableTourTarget 会因此判定目标不可用。引导跨路由推进步骤时
         * 恰好落在这一两帧上：一次性检查拿到 null 就永久跳过滚动，目标停在
         * 上一页遗留的滚动位置，高亮矩形被钳到视口边缘只露出一部分。
         *
         * 放在这里可以直接复用下方 pollAnimatedTarget 的重试生命周期——
         * 它本来就每帧重试直到目标可解析。也顺带修好"主锚点比 fallback 晚
         * 挂载"的情形：身份从 fallback 切到精确锚点时会重新滚动。
         *
         * 不需要"是否已滚动"标志：这个分支只在元素身份变化时进入，
         * 用户手动滚动只触发 measure 而不改变身份，因此不会和用户抢滚动条。
         */
        if (target) {
          scrollTargetIntoSafeViewport(target);
        }
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
