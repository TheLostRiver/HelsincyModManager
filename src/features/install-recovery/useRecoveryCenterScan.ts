import { useCallback, useEffect, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { scanInstallRecovery } from "../mods/modInstallPlanApi";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";
import {
  deriveRecoveryCenterViewModel,
  type RecoveryCenterViewModel,
} from "./recoveryCenterViewModel";

export type RecoveryCenterScanState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; viewModel: RecoveryCenterViewModel }
  | { status: "unavailable" };

type UseRecoveryCenterScanInput = {
  gameId: GameId;
  enabled: boolean;
};

export function useRecoveryCenterScan(input: UseRecoveryCenterScanInput) {
  const { activeProfile, activeProfileId } = useActiveProfile();
  const [state, setState] = useState<RecoveryCenterScanState>({ status: "idle" });
  const [refreshToken, setRefreshToken] = useState(0);

  const refresh = useCallback(() => {
    setRefreshToken((current) => current + 1);
  }, []);

  useEffect(() => {
    if (!input.enabled || activeProfile.status !== "ready" || activeProfileId === null) {
      setState({ status: "idle" });
      return undefined;
    }

    let cancelled = false;
    setState({ status: "loading" });

    void scanInstallRecovery({
      gameId: input.gameId,
      profileId: activeProfileId,
      modIds: [],
    })
      .then((summaries) => {
        if (!cancelled) {
          setState({ status: "ready", viewModel: deriveRecoveryCenterViewModel(summaries) });
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
  }, [activeProfile.status, activeProfileId, input.enabled, input.gameId, refreshToken]);

  return {
    state,
    refresh,
  };
}
