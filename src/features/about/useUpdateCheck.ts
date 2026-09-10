import { useCallback, useEffect, useRef, useState } from "react";

import { checkAppUpdate } from "./updateCheckApi.ts";
import { shouldCheckForUpdate } from "./updateCheckPolicy.ts";
import { readUpdateCheckPreference, writeUpdateCheckPreference } from "./updateCheckStorage.ts";
import { normalizeAppUpdateStatus, type AppUpdateStatusDto, type UpdateCheckPreference } from "./updateCheckTypes.ts";

export const UPDATE_CHECK_UI_TIMEOUT_MILLIS = 10_000;

export type UpdateCheckController = {
  /** 是否正在查询。用于显示「正在检查更新…」。 */
  checking: boolean;
  /** 后端返回的事实；`null` 表示没有结论（非 Tauri 环境或未到查询时机）。 */
  status: AppUpdateStatusDto | null;
  /** 最近一次查询未能得到可靠结论，不能用上次结果冒充本次成功。 */
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
 * - 挂载时按策略查询一次，不阻塞渲染；手动检查总有明确结果；
 * - 未知、超时、失败都撤下旧结论，可重试但不自动循环；
 * - 「是否自动检查」与「上次查询时刻」都是前端偏好，后端不保存。
 */
export function useUpdateCheck(): UpdateCheckController {
  const [preference, setPreference] = useState<UpdateCheckPreference>(readUpdateCheckPreference);
  const [checking, setChecking] = useState(false);
  const [status, setStatus] = useState<AppUpdateStatusDto | null>(null);
  const [attemptFailed, setAttemptFailed] = useState(false);
  const preferenceRef = useRef(preference);
  const mountedRef = useRef(false);
  const inFlightRef = useRef<symbol | null>(null);
  const timeoutRef = useRef<number | null>(null);

  const savePreference = useCallback((next: UpdateCheckPreference) => {
    preferenceRef.current = next;
    setPreference(next);
    writeUpdateCheckPreference(next);
  }, []);

  const finishCheck = useCallback((request: symbol, value: unknown) => {
    if (!mountedRef.current || inFlightRef.current !== request) return;
    inFlightRef.current = null;
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    timeoutRef.current = null;
    setChecking(false);
    const result = normalizeAppUpdateStatus(value);
    setStatus(result);
    setAttemptFailed(result === null || result.status === "unknown");
    if (result !== null && result.status !== "unknown") {
      savePreference({ ...preferenceRef.current, lastCheckedAt: Date.now() });
    }
  }, [savePreference]);

  const armTimeout = useCallback((request: symbol) => {
    timeoutRef.current = window.setTimeout(() => finishCheck(request, null), UPDATE_CHECK_UI_TIMEOUT_MILLIS);
  }, [finishCheck]);

  const runCheck = useCallback(() => {
    if (!mountedRef.current || inFlightRef.current !== null) return;
    const request = Symbol();
    inFlightRef.current = request;
    setChecking(true);
    armTimeout(request);
    void Promise.resolve().then(checkAppUpdate).then(
      (result) => finishCheck(request, result),
      () => finishCheck(request, null),
    );
  }, [armTimeout, finishCheck]);

  useEffect(() => {
    mountedRef.current = true;
    if (inFlightRef.current !== null) {
      // StrictMode 重挂 effect 时复用原请求，只恢复它的超时保护。
      armTimeout(inFlightRef.current);
    } else if (shouldCheckForUpdate(readUpdateCheckPreference(), Date.now())) {
      runCheck();
    }
    return () => {
      mountedRef.current = false;
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    };
  }, [armTimeout, runCheck]);

  const setAutoCheckEnabled = useCallback((enabled: boolean) => {
    if (!mountedRef.current) return;
    savePreference({ ...preferenceRef.current, autoCheckEnabled: enabled });
  }, [savePreference]);

  return {
    checking,
    status,
    attemptFailed,
    autoCheckEnabled: preference.autoCheckEnabled,
    setAutoCheckEnabled,
    refresh: runCheck,
  };
}
