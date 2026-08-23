import { useCallback, useEffect, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { scanInstallRecovery } from "../mods/modInstallPlanApi";
import type { InstallRecoverySummary } from "../mods/modInstallPlanTypes";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";

// state 只存后端语义摘要；带文案的 viewModel 由页面在渲染时结合当前 locale 派生，
// 语言切换不触发重新扫描。
export type RecoveryCenterScanState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; summaries: InstallRecoverySummary[] }
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
          setState({ status: "ready", summaries });
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
