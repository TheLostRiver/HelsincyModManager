import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  getActiveProfile,
  setActiveProfile as setActiveProfileCommand,
} from "./profileApi";
import type { Profile } from "./profileTypes";

export type ActiveProfileState =
  | { status: "loading" }
  | { status: "ready"; profile: Profile }
  | { status: "unavailable" };

type ActiveProfileContextValue = {
  activeProfile: ActiveProfileState;
  activeProfileId: string | null;
  refreshActiveProfile: () => void;
  setActiveProfile: (profileId: string) => Promise<void>;
};

const ActiveProfileContext = createContext<ActiveProfileContextValue | null>(null);

type ActiveProfileProviderProps = {
  children: ReactNode;
};

export function ActiveProfileProvider({ children }: ActiveProfileProviderProps) {
  const [activeProfile, setActiveProfileState] = useState<ActiveProfileState>({ status: "loading" });
  const [refreshToken, setRefreshToken] = useState(0);

  const refreshActiveProfile = useCallback(() => {
    setRefreshToken((current) => current + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setActiveProfileState({ status: "loading" });

    void getActiveProfile()
      .then((profile) => {
        if (!cancelled) {
          setActiveProfileState({ status: "ready", profile });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setActiveProfileState({ status: "unavailable" });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [refreshToken]);

  const setActiveProfile = useCallback(
    async (profileId: string) => {
      await setActiveProfileCommand(profileId);
      refreshActiveProfile();
    },
    [refreshActiveProfile],
  );

  const activeProfileId = activeProfile.status === "ready" ? activeProfile.profile.id : null;

  const value = useMemo<ActiveProfileContextValue>(
    () => ({
      activeProfile,
      activeProfileId,
      refreshActiveProfile,
      setActiveProfile,
    }),
    [activeProfile, activeProfileId, refreshActiveProfile, setActiveProfile],
  );

  return <ActiveProfileContext.Provider value={value}>{children}</ActiveProfileContext.Provider>;
}

export function useActiveProfile() {
  const context = useContext(ActiveProfileContext);

  if (!context) {
    throw new Error("useActiveProfile must be used inside ActiveProfileProvider.");
  }

  return context;
}
