import { useEffect, useState, useSyncExternalStore } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { getModReplacementSummary } from "../replacements/replacementApi";
import type { ModReplacementSummary } from "../replacements/replacementTypes";
import { useModLibrarySessionCache } from "./ModLibrarySessionCacheProvider";
import { getModDetail } from "./modLibraryApi";
import { coherentHoverReplacement } from "./modHoverModel";
import type { ModDetail } from "./modLibraryTypes";

type HoverDetails = {
  key: string;
  status: "loading" | "ready" | "unavailable";
  detail: ModDetail | null;
  replacement: ModReplacementSummary | null;
  replacementLoading: boolean;
};

export const MOD_HOVER_TIMEOUT_MILLIS = 15_000;

export function useModHoverDetails(modId: string, gameId: GameId, profileId: string | null) {
  const cache = useModLibrarySessionCache();
  const generation = useSyncExternalStore(cache.subscribe, cache.getGeneration, cache.getGeneration);
  const key = JSON.stringify([modId, gameId, profileId, generation]);
  const [result, setResult] = useState<HoverDetails | null>(null);

  useEffect(() => {
    let active = true;
    let detail: ModDetail | null = null;
    let summary: ModReplacementSummary | null = null;
    let detailDone = false;
    let summaryDone = false;
    const publish = () => {
      if (!active || cache.getGeneration() !== generation) return;
      if (detailDone && summaryDone) window.clearTimeout(timeout);
      setResult({ key, status: !detailDone ? "loading" : detail ? "ready" : "unavailable", detail,
        replacement: coherentHoverReplacement(detail, summary, gameId), replacementLoading: !summaryDone });
    };
    const timeout = window.setTimeout(() => {
      detailDone = true;
      summaryDone = true;
      publish();
      active = false;
    }, MOD_HOVER_TIMEOUT_MILLIS);
    // 延后到已提交 effect；StrictMode 的试挂载不会触发一次多余扫描。
    void Promise.resolve().then(() => {
      if (!active) return;
      void Promise.resolve().then(() => getModDetail({ modId })).then(
        (value) => { detail = value?.id === modId ? value : null; detailDone = true; publish(); },
        () => { detailDone = true; publish(); },
      );
      void Promise.resolve().then(() => getModReplacementSummary({ gameId, profileId, modId })).then(
        (value) => { summary = value; summaryDone = true; publish(); },
        () => { summaryDone = true; publish(); },
      );
    });
    return () => { active = false; window.clearTimeout(timeout); };
  }, [cache, gameId, generation, key, modId, profileId]);

  return result?.key === key ? result : { status: "loading" as const, detail: null, replacement: null, replacementLoading: true };
}
