import { useEffect, useRef } from "react";

const HOVER_GRACE_MS = 2000;

/** 普通计时与鼠标阅读宽限分别累计消耗；只有键盘焦点可以持续暂停。 */
export function useToastLifetime({
  durationMs,
  hovered,
  focused,
  exiting,
  onElapsed,
}: {
  durationMs: number;
  hovered: boolean;
  focused: boolean;
  exiting: boolean;
  onElapsed: () => void;
}) {
  const remainingMs = useRef(durationMs);
  const hoverGraceMs = useRef(HOVER_GRACE_MS);

  useEffect(() => {
    if (focused || exiting || durationMs <= 0) return undefined;

    const hoverAllowance = hovered ? hoverGraceMs.current : 0;
    const startedAt = performance.now();
    const timer = window.setTimeout(onElapsed, Math.max(0, remainingMs.current + hoverAllowance));

    return () => {
      window.clearTimeout(timer);
      const elapsed = Math.max(0, performance.now() - startedAt);
      const hoverElapsed = Math.min(hoverAllowance, elapsed);
      hoverGraceMs.current = Math.max(0, hoverGraceMs.current - hoverElapsed);
      remainingMs.current = Math.max(0, remainingMs.current - (elapsed - hoverElapsed));
    };
  }, [durationMs, exiting, focused, hovered, onElapsed]);
}
