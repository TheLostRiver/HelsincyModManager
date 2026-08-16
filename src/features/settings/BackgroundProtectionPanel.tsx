import {
  CheckCircle2,
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
  peekBackgroundProtectionControlStatus,
} from "./backgroundProtectionApi";
import {
  BackgroundProtectionAutoVerificationScheduler,
  type BackgroundProtectionAutoVerificationDecision,
} from "./backgroundProtectionAutoVerification";
import {
  getBackgroundProtectionCopy,
  getBackgroundProtectionErrorCode,
  getBackgroundProtectionErrorMessage,
  formatBackgroundProtectionDuration,
  hasBackgroundProtectionConverged,
  type BackgroundProtectionControlDto,
  type BackgroundProtectionTone,
} from "./backgroundProtectionTypes";
import {
  preserveBackgroundProtectionStateAfterRefreshFailure,
  readyBackgroundProtectionPanelState,
  type BackgroundProtectionPanelState,
} from "./backgroundProtectionPanelState";
import { useFeedback } from "../../shared/feedback";

type BusyAction = "refresh" | "enable" | "disable" | null;
type ActiveBusyAction = Exclude<BusyAction, null>;
type OperationOutcome = "success" | "reconciled" | "failed";
type RefreshSource = "manual" | "automatic";

type OperationToken = {
  action: ActiveBusyAction;
  generation: number;
  startedAt: number;
};

type CompletedOperation = {
  action: ActiveBusyAction;
  elapsedMs: number;
  outcome: OperationOutcome;
};

const OPERATION_TIMER_INTERVAL_MS = 100;

let retainedPanelState: BackgroundProtectionPanelState | null = null;

function initialPanelState(): BackgroundProtectionPanelState {
  const cachedControl = peekBackgroundProtectionControlStatus();
  if (
    cachedControl &&
    retainedPanelState?.status === "ready" &&
    retainedPanelState.control !== cachedControl
  ) {
    return readyBackgroundProtectionPanelState(cachedControl);
  }
  if (retainedPanelState) return retainedPanelState;
  return cachedControl
    ? readyBackgroundProtectionPanelState(cachedControl)
    : { status: "loading" };
}

function latestKnownPanelState(
  fallback: BackgroundProtectionPanelState,
): BackgroundProtectionPanelState {
  if (retainedPanelState?.status === "ready") return retainedPanelState;
  const cachedControl = peekBackgroundProtectionControlStatus();
  return cachedControl ? readyBackgroundProtectionPanelState(cachedControl) : fallback;
}

export function BackgroundProtectionPanel() {
  const { pushToast } = useFeedback();
  const [state, setState] = useState<BackgroundProtectionPanelState>(initialPanelState);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [operationElapsedMs, setOperationElapsedMs] = useState(0);
  const [lastOperation, setLastOperation] = useState<CompletedOperation | null>(null);
  const [autoVerificationActive, setAutoVerificationActive] = useState(false);
  const mountedRef = useRef(false);
  const busyRef = useRef(false);
  const requestGenerationRef = useRef(0);
  const activeOperationRef = useRef<OperationToken | null>(null);
  const automaticRefreshRef = useRef<
    () => Promise<BackgroundProtectionAutoVerificationDecision>
  >(async () => "complete");
  const autoVerificationSchedulerRef =
    useRef<BackgroundProtectionAutoVerificationScheduler | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    autoVerificationSchedulerRef.current = new BackgroundProtectionAutoVerificationScheduler({
      verify: () => automaticRefreshRef.current(),
      isBusy: () => busyRef.current,
      onActiveChange: (active) => {
        if (mountedRef.current) setAutoVerificationActive(active);
      },
    });
    const cleanup = () => {
      mountedRef.current = false;
      requestGenerationRef.current += 1;
      autoVerificationSchedulerRef.current?.dispose();
      autoVerificationSchedulerRef.current = null;
    };
    if (retainedPanelState) {
      return cleanup;
    }
    const generation = ++requestGenerationRef.current;

    void getBackgroundProtectionControlStatus()
      .then((control) => {
        if (mountedRef.current && generation === requestGenerationRef.current) {
          const nextState = readyBackgroundProtectionPanelState(control);
          retainedPanelState = nextState;
          setState(nextState);
        }
      })
      .catch((error: unknown) => {
        if (mountedRef.current && generation === requestGenerationRef.current) {
          const nextState = preserveBackgroundProtectionStateAfterRefreshFailure(
            latestKnownPanelState({ status: "loading" }),
            getBackgroundProtectionErrorCode(error),
          );
          retainedPanelState = nextState;
          setState(nextState);
        }
      });

    return cleanup;
  }, []);

  useEffect(() => {
    if (!busyAction) return;

    const updateElapsed = () => {
      const activeOperation = activeOperationRef.current;
      if (activeOperation && mountedRef.current) {
        setOperationElapsedMs(performance.now() - activeOperation.startedAt);
      }
    };
    updateElapsed();
    const timer = window.setInterval(updateElapsed, OPERATION_TIMER_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [busyAction]);

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
  const refreshWarningCode = state.status === "ready" ? state.refreshWarningCode : null;
  const visibleErrorMessage = visibleErrorCode
    ? getBackgroundProtectionErrorMessage(visibleErrorCode)
    : null;
  const switchChecked = unsupported
    ? false
    : busyAction === "enable"
      ? true
      : busyAction === "disable"
        ? false
        : (control?.desiredEnabled ?? false);
  const switchDisabled = busy || state.status !== "ready" || unsupported;

  const beginOperation = (action: ActiveBusyAction): OperationToken | null => {
    if (busyRef.current) return null;
    busyRef.current = true;
    const operation: OperationToken = {
      action,
      generation: ++requestGenerationRef.current,
      startedAt: performance.now(),
    };
    activeOperationRef.current = operation;
    setOperationElapsedMs(0);
    setLastOperation(null);
    setBusyAction(action);
    return operation;
  };

  const finishOperation = (operation: OperationToken, outcome: OperationOutcome) => {
    if (operation.generation !== requestGenerationRef.current) return;
    const elapsedMs = performance.now() - operation.startedAt;
    busyRef.current = false;
    activeOperationRef.current = null;
    if (mountedRef.current) {
      setOperationElapsedMs(elapsedMs);
      setLastOperation({ action: operation.action, elapsedMs, outcome });
      setBusyAction(null);
    }
  };

  const cancelStartingAutoRefresh = () => {
    autoVerificationSchedulerRef.current?.cancel();
  };

  const armStartingAutoRefresh = (control: BackgroundProtectionControlDto) => {
    if (control.status !== "starting" || !control.desiredEnabled) {
      cancelStartingAutoRefresh();
      return;
    }
    autoVerificationSchedulerRef.current?.arm();
  };

  const refreshStatus = async (
    source: RefreshSource = "manual",
  ): Promise<BackgroundProtectionControlDto | null> => {
    const operation = beginOperation("refresh");
    if (operation === null) return null;
    let outcome: OperationOutcome = "failed";
    let refreshedControl: BackgroundProtectionControlDto | null = null;

    try {
      const nextControl = await getBackgroundProtectionControlStatus({ force: true });
      if (mountedRef.current && operation.generation === requestGenerationRef.current) {
        const nextState = readyBackgroundProtectionPanelState(nextControl);
        retainedPanelState = nextState;
        setState(nextState);
        refreshedControl = nextControl;
        outcome = "success";
        if (
          source === "manual" &&
          autoVerificationSchedulerRef.current?.isActive() &&
          (nextControl.status !== "starting" || !nextControl.desiredEnabled)
        ) {
          cancelStartingAutoRefresh();
        }
        const refreshedCopy = getBackgroundProtectionCopy(nextControl.status);
        if (source === "manual" || nextControl.status !== "starting") {
          pushToast({
            eventKey:
              source === "automatic"
                ? "background-protection-auto-refreshed"
                : "background-protection-refreshed",
            title:
              source === "automatic" ? "后台保护自动验证已完成" : "后台保护状态已更新",
            message: `${refreshedCopy.description}，本次检查耗时 ${formatBackgroundProtectionDuration(performance.now() - operation.startedAt)}。`,
            tone: refreshedCopy.tone === "danger" ? "warning" : refreshedCopy.tone,
          });
        }
      }
    } catch (error) {
      if (mountedRef.current && operation.generation === requestGenerationRef.current) {
        const errorCode = getBackgroundProtectionErrorCode(error);
        const knownState = latestKnownPanelState(state);
        const preservedAuthoritativeState = knownState.status === "ready";
        const nextState = preserveBackgroundProtectionStateAfterRefreshFailure(
          knownState,
          errorCode,
        );
        retainedPanelState = nextState;
        setState(nextState);
        pushToast({
          eventKey: "background-protection-refresh-failed",
          title: "后台保护状态检查失败",
          message: preservedAuthoritativeState
            ? `${source === "automatic" ? "自动复查未完成，后续复查仍会继续" : "本次检查未完成，仍显示最近一次成功确认的状态；可稍后重试"}。耗时 ${formatBackgroundProtectionDuration(performance.now() - operation.startedAt)}。`
            : `${getBackgroundProtectionErrorMessage(errorCode)} 本次检查耗时 ${formatBackgroundProtectionDuration(performance.now() - operation.startedAt)}。`,
          tone: preservedAuthoritativeState ? "warning" : "danger",
        });
      }
    } finally {
      finishOperation(operation, outcome);
    }
    return refreshedControl;
  };

  automaticRefreshRef.current = async () => {
    const nextControl = await refreshStatus("automatic");
    if (!nextControl) return "continue";
    return nextControl.status === "starting" && nextControl.desiredEnabled
      ? "continue"
      : "complete";
  };

  const changeProtection = async (desiredEnabled: boolean) => {
    const operationToken = beginOperation(desiredEnabled ? "enable" : "disable");
    if (operationToken === null) return;
    let outcome: OperationOutcome = "failed";

    const operation = desiredEnabled ? enableBackgroundProtection : disableBackgroundProtection;
    try {
      const nextControl = await operation();
      if (mountedRef.current && operationToken.generation === requestGenerationRef.current) {
        const nextState = readyBackgroundProtectionPanelState(nextControl);
        retainedPanelState = nextState;
        setState(nextState);
        outcome = "success";
        if (desiredEnabled) armStartingAutoRefresh(nextControl);
        else cancelStartingAutoRefresh();
        pushToast({
          eventKey: desiredEnabled
            ? "background-protection-enabled"
            : "background-protection-disabled",
          title: desiredEnabled ? "后台保护已启用" : "后台保护已关闭",
          message: desiredEnabled
            ? nextControl.status === "protected"
              ? `系统任务与最近一次后台运行均已验证。耗时 ${formatBackgroundProtectionDuration(performance.now() - operationToken.startedAt)}。`
              : `系统任务已更新，HMM 将立即自动复查并等待首次后台运行验证；无需再次点击检查。耗时 ${formatBackgroundProtectionDuration(performance.now() - operationToken.startedAt)}。`
            : `退出 HMM 后不再由系统任务检查自动备份。耗时 ${formatBackgroundProtectionDuration(performance.now() - operationToken.startedAt)}。`,
          tone: desiredEnabled ? "success" : "neutral",
        });
      }
    } catch (error) {
      const actionErrorCode = getBackgroundProtectionErrorCode(error);
      if (mountedRef.current && operationToken.generation === requestGenerationRef.current) {
        let reconciledControl: BackgroundProtectionControlDto | null = null;
        try {
          reconciledControl = await getBackgroundProtectionControlStatus({ force: true });
          if (mountedRef.current && operationToken.generation === requestGenerationRef.current) {
            const converged = hasBackgroundProtectionConverged(reconciledControl, desiredEnabled);
            const nextState = readyBackgroundProtectionPanelState(
              reconciledControl,
              converged ? null : actionErrorCode,
            );
            retainedPanelState = nextState;
            setState(nextState);
            if (converged) {
              outcome = "reconciled";
              if (desiredEnabled) armStartingAutoRefresh(reconciledControl);
              else cancelStartingAutoRefresh();
              pushToast({
                eventKey: "background-protection-change-reconciled",
                title: desiredEnabled ? "后台保护已启用" : "后台保护已关闭",
                message: `操作确认曾短暂中断，但系统状态已自动重新读取，无需再次检查。耗时 ${formatBackgroundProtectionDuration(performance.now() - operationToken.startedAt)}。`,
                tone: "warning",
              });
            }
          }
        } catch {
          if (mountedRef.current && operationToken.generation === requestGenerationRef.current) {
            const nextState = preserveBackgroundProtectionStateAfterRefreshFailure(
              latestKnownPanelState(state),
              actionErrorCode,
            );
            retainedPanelState = nextState;
            setState(nextState);
          }
        }
        if (
          mountedRef.current &&
          operationToken.generation === requestGenerationRef.current &&
          outcome === "failed"
        ) {
          pushToast({
            eventKey: "background-protection-change-failed",
            title: desiredEnabled ? "后台保护启用失败" : "后台保护关闭失败",
            message: `${getBackgroundProtectionErrorMessage(actionErrorCode)} 耗时 ${formatBackgroundProtectionDuration(performance.now() - operationToken.startedAt)}。`,
            tone: "danger",
          });
        }
      }
    } finally {
      finishOperation(operationToken, outcome);
    }
  };

  const summary =
    state.status === "ready"
      ? getBackgroundProtectionCopy(state.control.status)
      : summaryForTransientState(state.status);
  const operationVisible = busy || lastOperation !== null;

  return (
    <div
      className="background-protection-panel"
      aria-busy={busy || state.status === "loading"}
      data-tour-id="settings.background-protection"
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
          onClick={() => void refreshStatus("manual")}
        >
          {busyAction === "refresh" ? (
            <LoaderCircle className="background-protection-spinner" size={14} aria-hidden="true" />
          ) : (
            <RefreshCw size={14} aria-hidden="true" />
          )}
          {busyAction === "refresh" ? "检查中" : "重新检查"}
        </button>
      </div>

      <div className="setting-row background-protection-panel__toggle">
        <span className="setting-row__copy">
          <strong>退出后继续保护自动备份</strong>
          <span id="background-protection-toggle-description">
            由系统后台任务定期唤醒现有备份流程，不改变每个 Profile 的备份计划。
          </span>
        </span>
        <label
          className={`background-protection-panel__switch-control${switchDisabled ? " is-disabled" : ""}`}
          title={switchDisabled ? undefined : switchChecked ? "关闭后台保护" : "开启后台保护"}
        >
          <input
            type="checkbox"
            checked={switchChecked}
            disabled={switchDisabled}
            aria-label="退出后继续保护自动备份"
            aria-describedby="background-protection-toggle-description background-protection-operation-status"
            onChange={(event) => void changeProtection(event.currentTarget.checked)}
          />
          <span className="setting-switch" aria-hidden="true" />
        </label>
      </div>

      <div
        id="background-protection-operation-status"
        className={`background-protection-panel__operation${operationVisible ? " is-visible" : ""}${busy ? " is-busy" : lastOperation ? ` is-${lastOperation.outcome}` : ""}`}
        role="status"
        aria-live="polite"
      >
        {busy ? (
          <>
            <LoaderCircle className="background-protection-spinner" size={15} aria-hidden="true" />
            <span>
              {busyAction === "refresh"
                ? "正在检查系统任务状态，请稍候…"
                : busyAction === "enable"
                  ? "正在启用后台保护，请勿关闭 HMM…"
                  : "正在关闭后台保护，请勿关闭 HMM…"}
            </span>
            <span className="background-protection-panel__timer" aria-hidden="true">
              {formatBackgroundProtectionDuration(operationElapsedMs)}
            </span>
          </>
        ) : lastOperation ? (
          <>
            {lastOperation.outcome === "failed" ? (
              <ShieldAlert size={15} aria-hidden="true" />
            ) : (
              <CheckCircle2 size={15} aria-hidden="true" />
            )}
            <span>{completedOperationLabel(lastOperation)}</span>
            <span className="background-protection-panel__timer">
              耗时 {formatBackgroundProtectionDuration(lastOperation.elapsedMs)}
            </span>
          </>
        ) : (
          <span>操作就绪</span>
        )}
      </div>

      <div className="background-protection-panel__feedback">
        <div className="background-protection-panel__message">
          {showStartingHint ? (
            <p className="background-protection-panel__hint">
              {autoVerificationActive
                ? "HMM 正在自动复查；首次后台运行完成后会自动更新为已保护，无需重复点击。在此之前完全退出仍可能失去即时提醒。"
                : "后台任务正在等待首次运行验证；需要立即确认时可重新检查，在此之前完全退出仍可能失去即时提醒。"}
            </p>
          ) : null}

          {visibleErrorMessage ? (
            <div className="background-protection-panel__error" role="alert">
              <ShieldAlert size={16} aria-hidden="true" />
              <span>{visibleErrorMessage}</span>
            </div>
          ) : null}

          {refreshWarningCode ? (
            <div className="background-protection-panel__warning" role="status">
              <CircleAlert size={16} aria-hidden="true" />
              <span>
                本次检查未完成，当前仍显示最近一次成功确认的状态；可稍后重新检查，正在验证时的自动复查不受影响。
              </span>
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

function completedOperationLabel(operation: CompletedOperation): string {
  if (operation.outcome === "reconciled") return "系统状态已自动重新同步";
  if (operation.outcome === "failed") {
    return operation.action === "refresh"
      ? "后台保护检查未完成"
      : operation.action === "enable"
        ? "后台保护启用未完成"
        : "后台保护关闭未完成";
  }
  return operation.action === "refresh"
    ? "后台保护检查完成"
    : operation.action === "enable"
      ? "后台保护启用完成"
      : "后台保护关闭完成";
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
