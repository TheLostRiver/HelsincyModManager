import { useEffect, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { scanInstallRecovery } from "../mods/modInstallPlanApi";
import { deriveInstallRecoveryHealth, type InstallRecoveryHealth } from "./installRecoveryHealth";
import { subscribeInstallRecoveryRefresh } from "./installRecoveryRefresh";

const DEFAULT_INSTALL_PROFILE_ID = "default";

export type InstallRecoveryHealthLoadState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; health: InstallRecoveryHealth }
  | { status: "unavailable" };

type UseInstallRecoveryHealthInput = {
  gameId: GameId;
  enabled: boolean;
};

export function useInstallRecoveryHealth(input: UseInstallRecoveryHealthInput): InstallRecoveryHealthLoadState {
  const [state, setState] = useState<InstallRecoveryHealthLoadState>({ status: "idle" });
  const [refreshToken, setRefreshToken] = useState(0);

  useEffect(() => {
    if (!input.enabled) {
      return undefined;
    }

    return subscribeInstallRecoveryRefresh(() => {
      setRefreshToken((current) => current + 1);
    });
  }, [input.enabled]);

  useEffect(() => {
    if (!input.enabled) {
      setState({ status: "idle" });
      return undefined;
    }

    let cancelled = false;
    setState({ status: "loading" });

    void scanInstallRecovery({
      gameId: input.gameId,
      profileId: DEFAULT_INSTALL_PROFILE_ID,
      modIds: [],
    })
      .then((summaries) => {
        if (!cancelled) {
          setState({ status: "ready", health: deriveInstallRecoveryHealth(summaries) });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setState({ status: "unavailable" });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [input.enabled, input.gameId, refreshToken]);

  return state;
}
