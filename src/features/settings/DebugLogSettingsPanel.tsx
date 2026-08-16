import { AlertTriangle, LoaderCircle, RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { getDebugLogSettings, setDebugLogSettings } from "./debugLogSettingsApi";
import {
  getDebugLogErrorCode,
  getDebugLogErrorMessage,
  type DebugLogSettingsState,
} from "./debugLogSettingsTypes";

export function DebugLogSettingsPanel() {
  const [state, setState] = useState<DebugLogSettingsState>({ status: "loading" });
  const [saving, setSaving] = useState(false);
  const mountedRef = useRef(false);

  const load = () => {
    setState({ status: "loading" });
    void getDebugLogSettings()
      .then((settings) => {
        if (mountedRef.current) setState({ status: "ready", settings, errorCode: null });
      })
      .catch((error: unknown) => {
        if (mountedRef.current) setState({ status: "error", errorCode: getDebugLogErrorCode(error) });
      });
  };

  useEffect(() => {
    mountedRef.current = true;
    load();
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const update = (enabled: boolean) => {
    if (state.status !== "ready" || saving) return;
    setSaving(true);
    void setDebugLogSettings(enabled)
      .then((settings) => {
        if (mountedRef.current) setState({ status: "ready", settings, errorCode: null });
      })
      .catch((error: unknown) => {
        if (mountedRef.current && state.status === "ready") {
          setState({ ...state, errorCode: getDebugLogErrorCode(error) });
        }
      })
      .finally(() => {
        if (mountedRef.current) setSaving(false);
      });
  };

  if (state.status === "loading") {
    return (
      <div className="debug-log-settings-panel" role="status" aria-busy="true">
        <LoaderCircle className="debug-log-settings-panel__spinner" size={16} aria-hidden="true" />
        <span>正在读取调试日志设置…</span>
      </div>
    );
  }

  if (state.status === "error") {
    return (
      <div className="settings-callout debug-log-settings-panel__error" role="alert">
        <AlertTriangle size={16} strokeWidth={2.1} />
        <span>{getDebugLogErrorMessage(state.errorCode)}</span>
        <button type="button" onClick={load}>
          <RefreshCw size={14} aria-hidden="true" />
          重新检查
        </button>
      </div>
    );
  }

  const errorMessage = state.errorCode ? getDebugLogErrorMessage(state.errorCode) : null;
  return (
    <div className="debug-log-settings-panel" aria-busy={saving}>
      <label className="setting-row debug-log-settings-panel__toggle">
        <span className="setting-row__copy">
          <strong>启用调试日志</strong>
          <span>仅在开启后写入受控的 Debug 事件；不会记录原始路径、错误正文、Manifest、Hash 或 Mod 内容。</span>
        </span>
        <input
          type="checkbox"
          checked={state.settings.enabled}
          disabled={saving}
          aria-describedby="debug-log-settings-description"
          onChange={(event) => update(event.currentTarget.checked)}
        />
        <span className="setting-switch" aria-hidden="true" />
      </label>
      <span id="debug-log-settings-description" className="debug-log-settings-panel__status" role="status" aria-live="polite">
        {saving ? "正在保存…" : state.settings.enabled ? "已启用" : "已关闭"}
      </span>
      {errorMessage ? (
        <div className="settings-callout debug-log-settings-panel__error" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>{errorMessage}</span>
        </div>
      ) : null}
    </div>
  );
}
