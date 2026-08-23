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
import { resolveCopy, useI18n } from "../../shared/i18n";
import {
  backgroundProtectionCopy,
  type BackgroundProtectionCopyDict,
} from "./backgroundProtectionCopy";

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

  const { locale } = useI18n();
  const bpCopy = resolveCopy(backgroundProtectionCopy, locale);
  const busy = busyAction !== null;
  const control = state.status === "ready" ? state.control : null;
  const status = control?.status;
  const copy = control ? getBackgroundProtectionCopy(control.status, locale) : null;
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
    ? getBackgroundProtectionErrorMessage(visibleErrorCode, locale)
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
        const refreshedCopy = getBackgroundProtectionCopy(nextControl.status, locale);
        if (source === "manual" || nextControl.status !== "starting") {
          pushToast({
            eventKey:
              source === "automatic"
                ? "background-protection-auto-refreshed"
                : "background-protection-refreshed",
            title:
              source === "automatic" ? bpCopy.toast.autoRefreshedTitle : bpCopy.toast.refreshedTitle,
            message: bpCopy.toast.refreshedMessage(
              refreshedCopy.description,
              formatBackgroundProtectionDuration(performance.now() - operation.startedAt, locale),
            ),
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
        const failureDuration = formatBackgroundProtectionDuration(
          performance.now() - operation.startedAt,
          locale,
        );
        pushToast({
          eventKey: "background-protection-refresh-failed",
          title: bpCopy.toast.refreshFailedTitle,
          message: preservedAuthoritativeState
            ? source === "automatic"
              ? bpCopy.toast.refreshFailedPreservedAuto(failureDuration)
              : bpCopy.toast.refreshFailedPreservedManual(failureDuration)
            : bpCopy.toast.refreshFailedMessage(
                getBackgroundProtectionErrorMessage(errorCode, locale),
                failureDuration,
              ),
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
        const changeDuration = formatBackgroundProtectionDuration(
          performance.now() - operationToken.startedAt,
          locale,
        );
        pushToast({
          eventKey: desiredEnabled
            ? "background-protection-enabled"
            : "background-protection-disabled",
          title: desiredEnabled ? bpCopy.toast.enabledTitle : bpCopy.toast.disabledTitle,
          message: desiredEnabled
            ? nextControl.status === "protected"
              ? bpCopy.toast.enabledProtectedMessage(changeDuration)
              : bpCopy.toast.enabledStartingMessage(changeDuration)
            : bpCopy.toast.disabledMessage(changeDuration),
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
                title: desiredEnabled ? bpCopy.toast.enabledTitle : bpCopy.toast.disabledTitle,
                message: bpCopy.toast.reconciledMessage(
                  formatBackgroundProtectionDuration(
                    performance.now() - operationToken.startedAt,
                    locale,
                  ),
                ),
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
            title: desiredEnabled ? bpCopy.toast.enableFailedTitle : bpCopy.toast.disableFailedTitle,
            message: bpCopy.toast.changeFailedMessage(
              getBackgroundProtectionErrorMessage(actionErrorCode, locale),
              formatBackgroundProtectionDuration(
                performance.now() - operationToken.startedAt,
                locale,
              ),
            ),
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
      ? getBackgroundProtectionCopy(state.control.status, locale)
      : summaryForTransientState(state.status, bpCopy.panel);
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
          {busyAction === "refresh" ? bpCopy.panel.checking : bpCopy.panel.recheck}
        </button>
      </div>

      <div className="setting-row background-protection-panel__toggle">
        <span className="setting-row__copy">
          <strong>{bpCopy.panel.toggleTitle}</strong>
          <span id="background-protection-toggle-description">
            {bpCopy.panel.toggleDescription}
          </span>
        </span>
        <label
          className={`background-protection-panel__switch-control${switchDisabled ? " is-disabled" : ""}`}
          title={
            switchDisabled
              ? undefined
              : switchChecked
                ? bpCopy.panel.switchTitleDisable
                : bpCopy.panel.switchTitleEnable
          }
        >
          <input
            type="checkbox"
            checked={switchChecked}
            disabled={switchDisabled}
            aria-label={bpCopy.panel.toggleTitle}
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
                ? bpCopy.panel.busyRefresh
                : busyAction === "enable"
                  ? bpCopy.panel.busyEnable
                  : bpCopy.panel.busyDisable}
            </span>
            <span className="background-protection-panel__timer" aria-hidden="true">
              {formatBackgroundProtectionDuration(operationElapsedMs, locale)}
            </span>
          </>
        ) : lastOperation ? (
          <>
            {lastOperation.outcome === "failed" ? (
              <ShieldAlert size={15} aria-hidden="true" />
            ) : (
              <CheckCircle2 size={15} aria-hidden="true" />
            )}
            <span>{completedOperationLabel(lastOperation, bpCopy.panel.completed)}</span>
            <span className="background-protection-panel__timer">
              {bpCopy.panel.elapsed(
                formatBackgroundProtectionDuration(lastOperation.elapsedMs, locale),
              )}
            </span>
          </>
        ) : (
          <span>{bpCopy.panel.operationReady}</span>
        )}
      </div>

      <div className="background-protection-panel__feedback">
        <div className="background-protection-panel__message">
          {showStartingHint ? (
            <p className="background-protection-panel__hint">
              {autoVerificationActive ? bpCopy.panel.startingHintAuto : bpCopy.panel.startingHintManual}
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
              <span>{bpCopy.panel.refreshWarning}</span>
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
                  ? bpCopy.panel.enabling
                  : bpCopy.panel.disabling
                : state.control.desiredEnabled
                  ? bpCopy.panel.retryEnable
                  : bpCopy.panel.retryDisable}
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
                {busyAction === "disable" ? bpCopy.panel.disabling : bpCopy.panel.stopProtection}
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function completedOperationLabel(
  operation: CompletedOperation,
  completed: BackgroundProtectionCopyDict["panel"]["completed"],
): string {
  if (operation.outcome === "reconciled") return completed.reconciled;
  if (operation.outcome === "failed") {
    return operation.action === "refresh"
      ? completed.refreshFailed
      : operation.action === "enable"
        ? completed.enableFailed
        : completed.disableFailed;
  }
  return operation.action === "refresh"
    ? completed.refreshDone
    : operation.action === "enable"
      ? completed.enableDone
      : completed.disableDone;
}

function summaryForTransientState(
  status: "loading" | "error",
  panel: BackgroundProtectionCopyDict["panel"],
) {
  if (status === "loading") {
    return { ...panel.loading, tone: "neutral" as const };
  }

  return { ...panel.unavailable, tone: "danger" as const };
}

function statusIcon(tone: BackgroundProtectionTone, spinning: boolean): ReactNode {
  if (spinning) {
    return <LoaderCircle className="background-protection-spinner" size={14} aria-hidden="true" />;
  }
  if (tone === "success") return <ShieldCheck size={14} aria-hidden="true" />;
  if (tone === "danger") return <ShieldAlert size={14} aria-hidden="true" />;
  return <CircleAlert size={14} aria-hidden="true" />;
}
