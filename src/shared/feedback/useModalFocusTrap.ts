import { useEffect, useRef, type RefObject } from "react";
import { getFocusableElements, getTrappedFocusIndex, isTopmostModalSurface } from "./focusTrap";

type UseModalFocusTrapInput = {
  active: boolean;
  containerRef: RefObject<HTMLElement | null>;
  closeOnEscape: boolean;
  onRequestClose: () => void;
  focusKey?: unknown;
  initialFocusRef?: RefObject<HTMLElement | null>;
};

export function useModalFocusTrap({
  active,
  containerRef,
  closeOnEscape,
  onRequestClose,
  focusKey,
  initialFocusRef,
}: UseModalFocusTrapInput) {
  const restoreFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!active) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" && event.key !== "Tab") {
        return;
      }

      const container = containerRef.current;
      if (!container || !isTopmostModalSurface(container)) {
        return;
      }

      if (event.key === "Escape" && closeOnEscape) {
        event.preventDefault();
        event.stopPropagation();
        onRequestClose();
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const focusableElements = getFocusableElements(container);
      const activeElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      const currentIndex = activeElement ? focusableElements.indexOf(activeElement) : -1;
      const nextIndex = getTrappedFocusIndex({
        currentIndex,
        focusableCount: focusableElements.length,
        backwards: event.shiftKey,
      });

      if (nextIndex !== null) {
        event.preventDefault();
        const target = nextIndex === -1 ? container : focusableElements[nextIndex];
        target.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [active, closeOnEscape, containerRef, onRequestClose]);

  useEffect(() => {
    if (!active || typeof document === "undefined") {
      return undefined;
    }

    restoreFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const frameId = window.requestAnimationFrame(() => {
      const container = containerRef.current;
      if (!container || !isTopmostModalSurface(container)) {
        return;
      }

      const requestedTarget = initialFocusRef?.current;
      const focusableElements = getFocusableElements(container);
      const target = requestedTarget && focusableElements.includes(requestedTarget)
        ? requestedTarget
        : focusableElements[0] ?? container;
      target.focus();
    });

    return () => {
      window.cancelAnimationFrame(frameId);
      if (restoreFocusRef.current?.isConnected) {
        restoreFocusRef.current.focus();
      }
      restoreFocusRef.current = null;
    };
  }, [active, containerRef, focusKey, initialFocusRef]);
}
