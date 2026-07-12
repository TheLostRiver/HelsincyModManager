import {
  CircleAlert,
  LoaderCircle,
  Power,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  disableBackgroundProtection,
  enableBackgroundProtection,
  getBackgroundProtectionControlStatus,
} from "./backgroundProtectionApi";
import {
  getBackgroundProtectionCopy,
  getBackgroundProtectionErrorCode,
  getBackgroundProtectionErrorMessage,
  type BackgroundProtectionControlDto,
  type BackgroundProtectionTone,
} from "./backgroundProtectionTypes";

type BackgroundProtectionPanelState =
  | { status: "loading" }
  | {
      status: "ready";
      control: BackgroundProtectionControlDto;
      actionErrorCode: string | null;
    }
  | { status: "error"; errorCode: string };

type BusyAction = "refresh" | "enable" | "disable" | null;

export function BackgroundProtectionPanel() {
  const [state, setState] = useState<BackgroundProtectionPanelState>({ status: "loading" });
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const mountedRef = useRef(false);
  const busyRef = useRef(false);
  const requestGenerationRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    const generation = ++requestGenerationRef.current;

    void getBackgroundProtectionControlStatus()
      .then((control) => {
        if (mountedRef.current && generation === requestGenerationRef.current) {
          setState({ status: "ready", control, actionErrorCode: null });
        }
      })
      .catch((error: unknown) => {
        if (mountedRef.current && generation === requestGenerationRef.current) {
          setState({ status: "error", errorCode: getBackgroundProtectionErrorCode(error) });
        }
      });

    return () => {
      mountedRef.current = false;
      requestGenerationRef.current += 1;
    };
  }, []);

  const busy = busyAction !== null;
  const control = state.status === "ready" ? state.control : null;
  const status = control?.status;
  const copy = control ? getBackgroundProtectionCopy(control.status) : null;
  const unsupported = status === "unsupported_platform";
  const showStartingHint = status === "starting";
  const visibleErrorCode =
    state.status === "error"
      ? state.errorCode
      : state.status === "ready"
        ? state.actionErrorCode ?? state.control.lastErrorCode
        : null;
  const visibleErrorMessage = visibleErrorCode
    ? getBackgroundProtectionErrorMessage(visibleErrorCode)
    : null;

  const beginOperation = (action: Exclude<BusyAction, null>) => {
    if (busyRef.current) return null;
    busyRef.current = true;
    setBusyAction(action);
    return ++requestGenerationRef.current;
  };

  const finishOperation = (generation: number) => {
    if (generation !== requestGenerationRef.current) return;
    busyRef.current = false;
    if (mountedRef.current) setBusyAction(null);
  };

  const refreshStatus = async () => {
    const generation = beginOperation("refresh");
    if (generation === null) return;

    try {
      const nextControl = await getBackgroundProtectionControlStatus();
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setState({ status: "ready", control: nextControl, actionErrorCode: null });
      }
    } catch (error) {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setState({ status: "error", errorCode: getBackgroundProtectionErrorCode(error) });
      }
    } finally {
      finishOperation(generation);
    }
  };

  const changeProtection = async (desiredEnabled: boolean) => {
    const generation = beginOperation(desiredEnabled ? "enable" : "disable");
    if (generation === null) return;

    const operation = desiredEnabled ? enableBackgroundProtection : disableBackgroundProtection;
    try {
      const nextControl = await operation();
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setState({ status: "ready", control: nextControl, actionErrorCode: null });
      }
    } catch (error) {
      const actionErrorCode = getBackgroundProtectionErrorCode(error);
      if (mountedRef.current && generation === requestGenerationRef.current) {
        try {
          const reconciled = await getBackgroundProtectionControlStatus();
          if (mountedRef.current && generation === requestGenerationRef.current) {
            setState({ status: "ready", control: reconciled, actionErrorCode });
          }
        } catch {
          if (mountedRef.current && generation === requestGenerationRef.current) {
            setState({ status: "error", errorCode: actionErrorCode });
          }
        }
      }
    } finally {
      finishOperation(generation);
    }
  };

  const summary =
    state.status === "ready"
      ? getBackgroundProtectionCopy(state.control.status)
      : summaryForTransientState(state.status);

  return (
    <div
      className="background-protection-panel"
      aria-busy={busy || state.status === "loading"}
    >
      <div
        className="background-protection-panel__summary"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        <div className="background-protection-panel__heading">
          <span className={`background-protection-status is-${summary.tone}`}>
            {statusIcon(summary.tone, state.status === "loading" || showStartingHint)}
            {summary.label}
          </span>
          <p>{summary.description}</p>
        </div>
        <button
          type="button"
          className="background-protection-panel__refresh"
          disabled={busy || state.status === "loading"}
          onClick={() => void refreshStatus()}
        >
          {busyAction === "refresh" ? (
            <LoaderCircle className="background-protection-spinner" size={14} aria-hidden="true" />
          ) : (
            <RefreshCw size={14} aria-hidden="true" />
          )}
          {busyAction === "refresh" ? "检查中" : "重新检查"}
        </button>
      </div>

      <label className="setting-row background-protection-panel__toggle">
        <span className="setting-row__copy">
          <strong>退出后继续保护自动备份</strong>
          <span id="background-protection-toggle-description">
            由系统后台任务定期唤醒现有备份流程，不改变每个 Profile 的备份计划。
          </span>
        </span>
        <input
          type="checkbox"
          checked={unsupported ? false : (control?.desiredEnabled ?? false)}
          disabled={busy || state.status !== "ready" || unsupported}
          aria-describedby="background-protection-toggle-description"
          onChange={(event) => void changeProtection(event.currentTarget.checked)}
        />
        <span className="setting-switch" aria-hidden="true" />
      </label>

      <div className="background-protection-panel__feedback">
        <div className="background-protection-panel__message">
          {showStartingHint ? (
            <p className="background-protection-panel__hint">
              首次后台运行完成后，请点击“重新检查”确认是否已保护；在此之前完全退出仍可能失去即时提醒。
            </p>
          ) : null}

          {visibleErrorMessage ? (
            <div className="background-protection-panel__error" role="alert">
              <ShieldAlert size={16} aria-hidden="true" />
              <span>{visibleErrorMessage}</span>
            </div>
          ) : null}
        </div>

        {state.status === "ready" && copy?.action === "retry" ? (
          <div className="background-protection-panel__actions">
            <button
              type="button"
              className="background-protection-panel__action is-primary"
              disabled={busy}
              onClick={() => void changeProtection(state.control.desiredEnabled)}
            >
              {busyAction === (state.control.desiredEnabled ? "enable" : "disable") ? (
                <LoaderCircle className="background-protection-spinner" size={14} aria-hidden="true" />
              ) : (
                <RotateCcw size={14} aria-hidden="true" />
              )}
              {busyAction === (state.control.desiredEnabled ? "enable" : "disable")
                ? state.control.desiredEnabled
                  ? "正在启用"
                  : "正在停用"
                : state.control.desiredEnabled
                  ? "重试启用"
                  : "重试停用"}
            </button>
            {state.control.desiredEnabled ? (
              <button
                type="button"
                className="background-protection-panel__action"
                disabled={busy}
                onClick={() => void changeProtection(false)}
              >
                {busyAction === "disable" ? (
                  <LoaderCircle className="background-protection-spinner" size={14} aria-hidden="true" />
                ) : (
                  <Power size={14} aria-hidden="true" />
                )}
                {busyAction === "disable" ? "正在停用" : "停用保护"}
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function summaryForTransientState(status: "loading" | "error") {
  if (status === "loading") {
    return {
      label: "正在读取状态",
      description: "正在核对后台保护设置与最近运行状态。",
      tone: "neutral" as const,
    };
  }

  return {
    label: "状态不可用",
    description: "暂时无法确认退出客户端后的后台保护状态。",
    tone: "danger" as const,
  };
}

function statusIcon(tone: BackgroundProtectionTone, spinning: boolean): ReactNode {
  if (spinning) {
    return <LoaderCircle className="background-protection-spinner" size={14} aria-hidden="true" />;
  }
  if (tone === "success") return <ShieldCheck size={14} aria-hidden="true" />;
  if (tone === "danger") return <ShieldAlert size={14} aria-hidden="true" />;
  return <CircleAlert size={14} aria-hidden="true" />;
}
