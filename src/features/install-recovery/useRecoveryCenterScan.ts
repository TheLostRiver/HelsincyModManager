import { useCallback, useEffect, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { scanInstallRecovery } from "../mods/modInstallPlanApi";
import {
  deriveRecoveryCenterViewModel,
  type RecoveryCenterViewModel,
} from "./recoveryCenterViewModel";

const DEFAULT_INSTALL_PROFILE_ID = "default";

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
  const [state, setState] = useState<RecoveryCenterScanState>({ status: "idle" });
  const [refreshToken, setRefreshToken] = useState(0);

  const refresh = useCallback(() => {
    setRefreshToken((current) => current + 1);
  }, []);

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
  }, [input.enabled, input.gameId, refreshToken]);

  return {
    state,
    refresh,
  };
}
