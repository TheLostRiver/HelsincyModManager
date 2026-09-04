import { createContext, useContext, type ReactNode } from "react";
import { useModStorageSettingsState, type ModStorageSettingsState } from "./useModStorageSettings";

const ModStorageSettingsContext = createContext<ModStorageSettingsState | null>(null);

/**
 * #275: one shared copy of the Mod storage directory state. The settings section changes it,
 * the library page reads `writesFrozen` for its import / delete entry points, and a running
 * migration must keep reporting progress after the user leaves the settings page — so it sits
 * above the router, like the game setup state.
 */
export function ModStorageSettingsProvider({ children }: { children: ReactNode }) {
  const value = useModStorageSettingsState();
  return <ModStorageSettingsContext.Provider value={value}>{children}</ModStorageSettingsContext.Provider>;
}

export function useModStorageSettings(): ModStorageSettingsState {
  const value = useContext(ModStorageSettingsContext);
  if (!value) {
    throw new Error("useModStorageSettings must be used inside ModStorageSettingsProvider.");
  }
  return value;
}
