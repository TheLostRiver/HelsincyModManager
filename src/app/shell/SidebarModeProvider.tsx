import { flushSync } from "react-dom";
import { createContext, useCallback, useMemo, useState, type ReactNode } from "react";
import { defaultSidebarMode, type PersistedSidebarModeSettings, type SidebarMode } from "./sidebarTypes";

const storageKey = "helsincy.sidebar-mode";

type SidebarModeContextValue = {
  sidebarMode: SidebarMode;
  setSidebarMode: (mode: SidebarMode) => void;
  toggleSidebarMode: () => void;
};

export const SidebarModeContext = createContext<SidebarModeContextValue | null>(null);

type SidebarModeProviderProps = {
  children: ReactNode;
};

export function SidebarModeProvider({ children }: SidebarModeProviderProps) {
  const [sidebarMode, setSidebarModeState] = useState<SidebarMode>(readPersistedSidebarMode);

  const setSidebarMode = useCallback((mode: SidebarMode) => {
    setSidebarModeState(mode);
    writePersistedSidebarMode(mode);
  }, []);

  const toggleSidebarMode = useCallback(() => {
    const nextMode: SidebarMode = sidebarMode === "classic" ? "floating" : "classic";

    if ("startViewTransition" in document) {
      document.startViewTransition(() => {
        flushSync(() => {
          setSidebarModeState(nextMode);
          writePersistedSidebarMode(nextMode);
        });
      });
    } else {
      setSidebarModeState(nextMode);
      writePersistedSidebarMode(nextMode);
    }
  }, [sidebarMode]);

  const value = useMemo(
    () => ({ sidebarMode, setSidebarMode, toggleSidebarMode }),
    [setSidebarMode, sidebarMode, toggleSidebarMode],
  );

  return <SidebarModeContext.Provider value={value}>{children}</SidebarModeContext.Provider>;
}

function readPersistedSidebarMode(): SidebarMode {
  try {
    const rawValue = window.localStorage.getItem(storageKey);
    if (rawValue === null) {
      return defaultSidebarMode;
    }

    if (isSidebarMode(rawValue)) {
      return rawValue;
    }

    const parsedValue = JSON.parse(rawValue) as Partial<PersistedSidebarModeSettings>;
    return parsedValue.version === 1 && isSidebarMode(parsedValue.sidebarMode)
      ? parsedValue.sidebarMode
      : defaultSidebarMode;
  } catch {
    return defaultSidebarMode;
  }
}

function writePersistedSidebarMode(sidebarMode: SidebarMode) {
  try {
    const value: PersistedSidebarModeSettings = { version: 1, sidebarMode };
    window.localStorage.setItem(storageKey, JSON.stringify(value));
  } catch {
    return;
  }
}

function isSidebarMode(value: unknown): value is SidebarMode {
  return value === "classic" || value === "floating";
}
