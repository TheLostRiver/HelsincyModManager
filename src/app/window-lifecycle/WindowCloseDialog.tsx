import { Check, LoaderCircle, Minimize2, Power, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { appShellCopy, type AppShellCopy } from "../appShellCopy";
import type { AppExitBlockReason, SaveBackupExitGuardReason } from "./windowLifecycleApi";
import type { WindowClosePreference } from "./windowClosePreference";
import "./WindowCloseDialog.css";

export type WindowCloseDialogMode =
  | { kind: "normal" }
  | { kind: "unsafe"; reason: SaveBackupExitGuardReason; exitAuthorization: string }
  | { kind: "blocked"; reason: AppExitBlockReason };

type WindowCloseDialogProps = {
  mode: WindowCloseDialogMode | null;
  errorMessage: string | null;
  onCancel: () => void;
  onConfirm: (action: WindowClosePreference, remember: boolean) => Promise<void>;
};

type ExecutingAction = "tray" | "exit" | null;
type DialogPhase = "closed" | "opening" | "open" | "settled" | "closing";

const EXECUTION_FEEDBACK_DELAY_MS = 360;
const DIALOG_TRANSITION_MS = 200;
const REDUCED_MOTION_TRANSITION_MS = 140;
const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "a[href]",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function getFocusableDialogElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => !element.getAttribute("aria-hidden") && element.tabIndex >= 0,
  );
}

export function WindowCloseDialog({ mode, errorMessage, onCancel, onConfirm }: WindowCloseDialogProps) {
  const { locale } = useI18n();
  const copy: AppShellCopy["windowClose"] = resolveCopy(appShellCopy, locale).windowClose;
  const [renderedMode, setRenderedMode] = useState<WindowCloseDialogMode | null>(mode);
  const [phase, setPhase] = useState<DialogPhase>(mode ? "opening" : "closed");
  const [remember, setRemember] = useState(false);
  const [executing, setExecuting] = useState<ExecutingAction>(null);
  const [successText, setSuccessText] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const trayButtonRef = useRef<HTMLButtonElement>(null);
  const closeTimerRef = useRef<number | null>(null);
  const settleTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
    if (settleTimerRef.current !== null) {
      window.clearTimeout(settleTimerRef.current);
      settleTimerRef.current = null;
    }
    if (!mode) return;

    setRenderedMode(mode);
    setPhase("opening");
    setRemember(false);
    setExecuting(null);
    setSuccessText(null);
    let openFrame = 0;
    const openingFrame = window.requestAnimationFrame(() => {
      openFrame = window.requestAnimationFrame(() => {
        setPhase("open");
        settleTimerRef.current = window.setTimeout(() => {
          settleTimerRef.current = null;
          setPhase((currentPhase) => (currentPhase === "open" ? "settled" : currentPhase));
        }, getDialogTransitionMillis());
      });
    });
    const focusTimer = window.setTimeout(() => trayButtonRef.current?.focus(), 0);
    return () => {
      window.cancelAnimationFrame(openingFrame);
      window.cancelAnimationFrame(openFrame);
      window.clearTimeout(focusTimer);
      if (settleTimerRef.current !== null) {
        window.clearTimeout(settleTimerRef.current);
        settleTimerRef.current = null;
      }
    };
  }, [mode]);

  useEffect(
    () => () => {
      if (closeTimerRef.current !== null) window.clearTimeout(closeTimerRef.current);
      if (settleTimerRef.current !== null) window.clearTimeout(settleTimerRef.current);
    },
    [],
  );

  const requestCancel = useCallback(() => {
    if (executing || phase === "closing") return;
    if (settleTimerRef.current !== null) {
      window.clearTimeout(settleTimerRef.current);
      settleTimerRef.current = null;
    }
    setPhase("closing");
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      setRenderedMode(null);
      setPhase("closed");
      onCancel();
    }, getDialogTransitionMillis());
  }, [executing, onCancel, phase]);

  const execute = useCallback(
    async (action: "tray" | "exit") => {
      if (!renderedMode || phase === "closing") return;
      setExecuting(action);
      setSuccessText(action === "tray" ? copy.successTray : copy.successExit);
      try {
        await new Promise((resolve) => window.setTimeout(resolve, EXECUTION_FEEDBACK_DELAY_MS));
        await onConfirm(action, renderedMode.kind === "normal" ? remember : false);
        if (action === "tray") {
          setRenderedMode(null);
          setPhase("closed");
        }
      } catch {
        setExecuting(null);
        setSuccessText(null);
      }
    },
    [copy.successExit, copy.successTray, onConfirm, phase, remember, renderedMode],
  );

  useEffect(() => {
    if (!renderedMode) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !executing && phase !== "closing") {
        event.preventDefault();
        requestCancel();
        return;
      }

      if (
        event.key === "Enter" &&
        !event.altKey &&
        !event.ctrlKey &&
        !event.metaKey &&
        !event.shiftKey &&
        !event.repeat &&
        !event.isComposing &&
        !executing &&
        phase !== "closing"
      ) {
        const activeElement = document.activeElement;
        if (activeElement instanceof HTMLButtonElement && dialogRef.current?.contains(activeElement)) {
          const focusedAction = activeElement.dataset.closeAction;
          if (focusedAction !== "tray" && focusedAction !== "exit") return;
          event.preventDefault();
          void execute(focusedAction);
          return;
        }
        event.preventDefault();
        void execute("tray");
        return;
      }

      if (event.key !== "Tab") return;
      const dialog = dialogRef.current;
      if (!dialog) return;

      const focusableElements = getFocusableDialogElements(dialog);
      if (focusableElements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const firstFocusable = focusableElements[0];
      const lastFocusable = focusableElements[focusableElements.length - 1];
      const activeElement = document.activeElement;

      if (event.shiftKey) {
        if (activeElement === firstFocusable || !dialog.contains(activeElement)) {
          event.preventDefault();
          lastFocusable.focus();
        }
        return;
      }

      if (activeElement === lastFocusable || !dialog.contains(activeElement)) {
        event.preventDefault();
        firstFocusable.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [execute, executing, phase, renderedMode, requestCancel]);

  if (!renderedMode) return null;

  const transitionStyle = {
    "--window-close-transition-duration": `${getDialogTransitionMillis()}ms`,
  } as CSSProperties;
  const interactionsDisabled = Boolean(executing) || phase === "closing";

  return (
    <div
      className={`window-close-overlay is-${phase}`}
      style={transitionStyle}
      onMouseDown={(event) =>
        event.target === event.currentTarget && !interactionsDisabled && requestCancel()
      }
    >
      <div
        ref={dialogRef}
        className={`window-close-dialog is-${renderedMode.kind}`}
        role={renderedMode.kind === "normal" ? "dialog" : "alertdialog"}
        aria-modal="true"
        aria-labelledby="window-close-title"
        aria-describedby="window-close-description"
        tabIndex={-1}
      >
        <button
          className="window-close-dialog__close"
          type="button"
          onClick={requestCancel}
          disabled={interactionsDisabled}
          aria-label={copy.cancelCloseAria}
        >
          <X size={15} strokeWidth={2.2} />
        </button>

        <header className="window-close-dialog__header">
          <h2 id="window-close-title">
            {renderedMode.kind === "unsafe"
              ? copy.unsafeTitle
              : renderedMode.kind === "blocked"
                ? copy.blockedTitle
                : copy.normalTitle}
          </h2>
          <p id="window-close-description">
            {renderedMode.kind === "unsafe"
              ? copy.unsafeReasons[renderedMode.reason]
              : renderedMode.kind === "blocked"
                ? copy.blockedReasons[renderedMode.reason]
              : copy.normalDescription}
          </p>
        </header>

        {errorMessage ? <p className="window-close-dialog__error">{errorMessage}</p> : null}

        <div className="window-close-dialog__options">
          <button
            ref={trayButtonRef}
            className="window-close-option is-tray is-default"
            type="button"
            data-default-action="true"
            data-close-action="tray"
            onClick={() => void execute("tray")}
            disabled={interactionsDisabled}
          >
            <span className="window-close-option__icon" aria-hidden="true">
              <Minimize2 size={24} strokeWidth={2.15} />
            </span>
            <span className="window-close-option__copy">
              <strong>{renderedMode.kind === "unsafe" ? copy.trayStay : copy.trayCollapse}</strong>
              <span>
                {renderedMode.kind === "unsafe"
                  ? copy.trayUnsafeHint
                  : renderedMode.kind === "blocked"
                    ? copy.trayBlockedHint
                  : copy.trayNormalHint}
              </span>
            </span>
            {executing === "tray" ? <LoaderCircle className="window-close-option__spinner" size={22} /> : null}
          </button>

          {renderedMode.kind !== "blocked" ? (
            <button
              className="window-close-option is-exit"
              type="button"
              data-close-action="exit"
              onClick={() => void execute("exit")}
              disabled={interactionsDisabled}
            >
              <span className="window-close-option__icon" aria-hidden="true">
                <Power size={24} strokeWidth={2.15} />
              </span>
              <span className="window-close-option__copy">
                <strong>{renderedMode.kind === "unsafe" ? copy.exitStill : copy.exitFull}</strong>
                <span>
                  {renderedMode.kind === "unsafe"
                    ? copy.exitUnsafeHint
                    : copy.exitNormalHint}
                </span>
              </span>
              {executing === "exit" ? (
                <LoaderCircle className="window-close-option__spinner" size={22} />
              ) : null}
            </button>
          ) : null}
        </div>

        <footer className={`window-close-dialog__footer is-${renderedMode.kind}`}>
          {renderedMode.kind === "normal" ? (
            <label className="window-close-dialog__remember">
              <input
                type="checkbox"
                checked={remember}
                onChange={(event) => setRemember(event.target.checked)}
                disabled={interactionsDisabled}
              />
              <span className="window-close-dialog__checkbox" aria-hidden="true">
                <Check size={12} strokeWidth={2.6} />
              </span>
              <span>{copy.remember}</span>
            </label>
          ) : null}

          <button
            className="window-close-dialog__cancel"
            type="button"
            onClick={requestCancel}
            disabled={interactionsDisabled}
          >
            {renderedMode.kind === "unsafe"
              ? copy.cancelUnsafe
              : renderedMode.kind === "blocked"
                ? copy.cancelBlocked
                : copy.cancelNormal}
          </button>
        </footer>

        {successText ? (
          <div className="window-close-dialog__success" aria-live="polite">
            <span aria-hidden="true">
              <Check size={30} strokeWidth={3} />
            </span>
            <strong>{successText}</strong>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function getDialogTransitionMillis() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return DIALOG_TRANSITION_MS;
  }

  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ? REDUCED_MOTION_TRANSITION_MS
    : DIALOG_TRANSITION_MS;
}
