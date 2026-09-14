import { useCallback, useState } from "react";

const CARD_HOVER_STORAGE_KEY = "hmm.modLibrary.showCardHover";

function readInitialCardHoverVisibility() {
  try {
    return window.localStorage.getItem(CARD_HOVER_STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

export function useModCardHoverPreference() {
  const [showCardHover, setShowCardHover] = useState(readInitialCardHoverVisibility);
  const toggleCardHover = useCallback(() => {
    const nextValue = !showCardHover;
    try {
      window.localStorage.setItem(CARD_HOVER_STORAGE_KEY, String(nextValue));
    } catch {
      // Storage may be unavailable; the in-memory preference still works.
    }
    setShowCardHover(nextValue);
  }, [showCardHover]);

  return { showCardHover, toggleCardHover };
}
