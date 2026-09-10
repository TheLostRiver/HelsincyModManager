import { useCallback, useEffect, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { scanInstallRecovery } from "../mods/modInstallPlanApi";
import type { InstallRecoverySummary } from "../mods/modInstallPlanTypes";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";
import { getModDetail } from "../mods/modLibraryApi";

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
  const [modNames, setModNames] = useState<Record<string, string>>({});

  useEffect(() => { setModNames({}); }, [activeProfileId, input.gameId]);

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

    void Promise.resolve().then(() => cancelled ? null : scanInstallRecovery({
      gameId: input.gameId,
      profileId: activeProfileId,
      modIds: [],
    }))
      .then(async (summaries) => {
        if (!cancelled && summaries !== null) {
          setState({ status: "ready", summaries });
          // 名称仅用于展示；扫描结果先显示，至多四个元数据查询并行，不重扫 Mod 包。
          const pending = [...new Set(summaries.filter((item) => item.status !== "completed" && item.status !== "not_installed").map((item) => item.modId))];
          const names: [string, string][] = [];
          await Promise.all(Array.from({ length: Math.min(4, pending.length) }, async () => {
            while (!cancelled && pending.length > 0) {
              const modId = pending.shift()!;
              try {
                const detail = await getModDetail({ modId });
                if (detail?.id === modId && typeof detail.name === "string" && detail.name.trim()) names.push([modId, detail.name.trim()]);
              } catch { /* 名称读取失败不改变恢复事实，界面保留稳定 Mod ID。 */ }
            }
          }));
          if (!cancelled) setModNames((current) => ({ ...current, ...Object.fromEntries(names) }));
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
    modNames,
    refresh,
  };
}
