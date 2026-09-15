import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { useGameSetup } from "../game-setup/GameSetupProvider";
import { getModInstallationContext, type ModInstallationContext } from "./modInstallationApi";

export type ModInstallationState =
  | { status: "loading" }
  | { status: "ready"; context: ModInstallationContext }
  | { status: "unavailable"; code: string };

type ContextValue = {
  installationScope: ModInstallationState;
  installationScopeId: string | null;
  refreshInstallationScope: () => void;
};

const ModInstallationContextValue = createContext<ContextValue | null>(null);

export function ModInstallationProvider({ children }: { children: ReactNode }) {
  const { status: gameStatus } = useGameSetup();
  const [loaded, setLoaded] = useState<{ source: typeof gameStatus; state: ModInstallationState } | null>(null);
  const [refreshToken, setRefreshToken] = useState(0);
  const refreshInstallationScope = useCallback(() => setRefreshToken((value) => value + 1), []);

  useEffect(() => {
    if (gameStatus.kind !== "configured") return;
    let cancelled = false;
    setLoaded({ source: gameStatus, state: { status: "loading" } });
    // Deferral avoids duplicate registration during StrictMode's initial effect replay.
    void Promise.resolve().then(() => cancelled ? null : getModInstallationContext(gameStatus.gameId))
      .then((context) => {
        if (cancelled || context === null) return;
        if (context.gameId !== gameStatus.gameId || !context.scopeId || !context.installationId) {
          throw new Error("invalid installation context");
        }
        setLoaded({ source: gameStatus, state: { status: "ready", context } });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        const code = typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
          ? error.code : "mod_installation_scope_unavailable";
        setLoaded({ source: gameStatus, state: { status: "unavailable", code } });
      });
    return () => { cancelled = true; };
  }, [gameStatus, refreshToken]);

  const installationScope = useMemo<ModInstallationState>(() => gameStatus.kind !== "configured"
    ? { status: "unavailable", code: "mod_installation_game_unavailable" }
    : loaded?.source === gameStatus ? loaded.state : { status: "loading" }, [gameStatus, loaded]);
  const installationScopeId = installationScope.status === "ready" ? installationScope.context.scopeId : null;
  const value = useMemo(() => ({ installationScope, installationScopeId, refreshInstallationScope }),
    [installationScope, installationScopeId, refreshInstallationScope]);
  return <ModInstallationContextValue.Provider value={value}>{children}</ModInstallationContextValue.Provider>;
}

export function useModInstallation(): ContextValue {
  const value = useContext(ModInstallationContextValue);
  if (!value) throw new Error("useModInstallation must be used inside ModInstallationProvider.");
  return value;
}
