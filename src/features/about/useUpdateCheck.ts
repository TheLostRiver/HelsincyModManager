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
  /**
   * 最近一次查询是否失败。
   *
   * 只在**手里还有旧结论**时才有意义：此时仍展示旧结论，但必须同时说明
   * 「上次检查失败」——否则用户点了「检查更新」之后会以为复查通过了。
   */
  attemptFailed: boolean;
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
  const [attemptFailed, setAttemptFailed] = useState(false);
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
        // 没有结论：保留可能已有的旧结论（它曾是真的），但标记为「上次检查失败」，
        // 由界面一并说明。不弹错误、不打扰——失败本身仍然静默。
        setAttemptFailed(true);
        return;
      }
      setAttemptFailed(false);
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
    attemptFailed,
    autoCheckEnabled: preference.autoCheckEnabled,
    setAutoCheckEnabled,
    refresh: runCheck,
  };
}
