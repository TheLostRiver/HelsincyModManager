import { useCallback, useEffect, useRef, useState } from "react";

import { checkAppUpdate } from "./updateCheckApi.ts";
import { shouldCheckForUpdate } from "./updateCheckPolicy.ts";
import { readUpdateCheckPreference, writeUpdateCheckPreference } from "./updateCheckStorage.ts";
import type { AppUpdateStatusDto, UpdateCheckPreference } from "./updateCheckTypes.ts";

export type UpdateCheckController = {
  /** 是否正在查询。用于显示「正在检查更新…」。 */
  checking: boolean;
  /** 后端返回的事实；`null` 表示没有结论（非 Tauri 环境或未到查询时机）。 */
  status: AppUpdateStatusDto | null;
  autoCheckEnabled: boolean;
  setAutoCheckEnabled: (enabled: boolean) => void;
  /** 手动检查一次，忽略 24 小时节流。 */
  refresh: () => void;
};

/**
 * 「检查更新」的状态机。
 *
 * 行为约束（对应验收标准）：
 * - 挂载时按策略查询一次，且**不阻塞渲染**（异步，失败静默）；
 * - 查询失败不产生任何可展示的错误——`status` 保持 `null`，界面什么都不显示；
 * - 「是否自动检查」与「上次查询时刻」都是前端偏好，后端不保存。
 */
export function useUpdateCheck(): UpdateCheckController {
  const [preference, setPreference] = useState<UpdateCheckPreference>(readUpdateCheckPreference);
  const [checking, setChecking] = useState(false);
  const [status, setStatus] = useState<AppUpdateStatusDto | null>(null);
  const mountedRef = useRef(true);
  const inFlightRef = useRef(false);

  const runCheck = useCallback(() => {
    if (inFlightRef.current) {
      return;
    }
    inFlightRef.current = true;
    setChecking(true);

    void checkAppUpdate().then((result) => {
      inFlightRef.current = false;
      if (!mountedRef.current) {
        return;
      }
      setChecking(false);
      if (result === null) {
        // 没有结论就什么都别显示——断网、超时都是常态，不打扰用户。
        return;
      }
      setStatus(result);
      setPreference((current) => {
        const next: UpdateCheckPreference = { ...current, lastCheckedAt: Date.now() };
        writeUpdateCheckPreference(next);
        return next;
      });
    });
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    if (shouldCheckForUpdate(readUpdateCheckPreference(), Date.now())) {
      runCheck();
    }
    return () => {
      mountedRef.current = false;
    };
  }, [runCheck]);

  const setAutoCheckEnabled = useCallback((enabled: boolean) => {
    setPreference((current) => {
      const next: UpdateCheckPreference = { ...current, autoCheckEnabled: enabled };
      writeUpdateCheckPreference(next);
      return next;
    });
  }, []);

  return {
    checking,
    status,
    autoCheckEnabled: preference.autoCheckEnabled,
    setAutoCheckEnabled,
    refresh: runCheck,
  };
}
