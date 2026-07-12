import { Check, LoaderCircle, Minimize2, Power, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { AppExitGuardReason } from "./windowLifecycleApi";
import type { WindowClosePreference } from "./windowClosePreference";
import "./WindowCloseDialog.css";

export type WindowCloseDialogMode =
  | { kind: "normal" }
  | { kind: "unsafe"; reason: AppExitGuardReason };

type WindowCloseDialogProps = {
  mode: WindowCloseDialogMode | null;
  errorMessage: string | null;
  onCancel: () => void;
  onConfirm: (action: WindowClosePreference, remember: boolean) => Promise<void>;
};

type ExecutingAction = "tray" | "exit" | null;

const EXECUTION_FEEDBACK_DELAY_MS = 360;
const UNSAFE_EXIT_REASON_MESSAGES: Record<AppExitGuardReason, string> = {
  background_starting:
    "后台任务已注册，但尚未完成首次运行验证。Windows 仍会在约 1 分钟后尝试运行；若失败，应用退出后无法立即提醒你。",
  background_not_enabled: "后台保护尚未启用。完全退出后，自动备份不会继续按计划检查。",
  registration_failed: "后台任务注册或校验失败。完全退出后，自动备份可能不会按计划运行。",
  worker_unhealthy: "后台任务最近没有按预期运行。完全退出后，自动备份可能失去保护。",
  permission_required: "当前账户权限不足，后台任务无法完成注册或校验。",
  unsupported_platform: "当前平台不支持退出后的后台自动备份保护。",
  status_unavailable: "暂时无法确认后台保护状态。为避免静默失去保护，建议先留在托盘。",
};
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
  const [remember, setRemember] = useState(false);
  const [executing, setExecuting] = useState<ExecutingAction>(null);
  const [successText, setSuccessText] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const trayButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!mode) return;
    setRemember(false);
    setExecuting(null);
    setSuccessText(null);
    const focusTimer = window.setTimeout(() => {
      if (mode.kind === "unsafe") trayButtonRef.current?.focus();
      else dialogRef.current?.focus();
    }, 0);
    return () => window.clearTimeout(focusTimer);
  }, [mode]);

  useEffect(() => {
    if (!mode) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !executing) {
        onCancel();
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
  }, [executing, mode, onCancel]);

  if (!mode) return null;

  const execute = async (action: "tray" | "exit") => {
    setExecuting(action);
    setSuccessText(action === "tray" ? "已收起至系统托盘" : "正在退出应用");
    try {
      await new Promise((resolve) => window.setTimeout(resolve, EXECUTION_FEEDBACK_DELAY_MS));
      await onConfirm(action, mode.kind === "normal" ? remember : false);
    } catch {
      setExecuting(null);
      setSuccessText(null);
    }
  };

  return (
    <div
      className="window-close-overlay"
      onMouseDown={(event) => event.target === event.currentTarget && !executing && onCancel()}
    >
      <div
        ref={dialogRef}
        className={`window-close-dialog is-${mode.kind}`}
        role={mode.kind === "unsafe" ? "alertdialog" : "dialog"}
        aria-modal="true"
        aria-labelledby="window-close-title"
        aria-describedby="window-close-description"
        tabIndex={-1}
      >
        <button
          className="window-close-dialog__close"
          type="button"
          onClick={onCancel}
          disabled={Boolean(executing)}
          aria-label="取消关闭"
        >
          <X size={15} strokeWidth={2.2} />
        </button>

        <header className="window-close-dialog__header">
          <h2 id="window-close-title">{mode.kind === "unsafe" ? "后台保护尚未就绪" : "准备退出 Helsincy？"}</h2>
          <p id="window-close-description">
            {mode.kind === "unsafe"
              ? UNSAFE_EXIT_REASON_MESSAGES[mode.reason]
              : "请选择关闭主窗口时的操作。你也可以在设置里随时改回每次询问。"}
          </p>
        </header>

        {errorMessage ? <p className="window-close-dialog__error">{errorMessage}</p> : null}

        <div className="window-close-dialog__options">
          <button
            ref={trayButtonRef}
            className="window-close-option is-tray"
            type="button"
            onClick={() => void execute("tray")}
            disabled={Boolean(executing)}
          >
            <span className="window-close-option__icon" aria-hidden="true">
              <Minimize2 size={24} strokeWidth={2.15} />
            </span>
            <span className="window-close-option__copy">
              <strong>{mode.kind === "unsafe" ? "留在托盘" : "收起至系统托盘"}</strong>
              <span>
                {mode.kind === "unsafe"
                  ? "保留客户端运行，让自动备份继续在本次会话内检查。"
                  : "应用将在后台持续运行，自动备份仍会在客户端运行期间检查。"}
              </span>
            </span>
            {executing === "tray" ? <LoaderCircle className="window-close-option__spinner" size={22} /> : null}
          </button>

          <button
            className="window-close-option is-exit"
            type="button"
            onClick={() => void execute("exit")}
            disabled={Boolean(executing)}
          >
            <span className="window-close-option__icon" aria-hidden="true">
              <Power size={24} strokeWidth={2.15} />
            </span>
            <span className="window-close-option__copy">
              <strong>{mode.kind === "unsafe" ? "仍然退出" : "完全退出应用程序"}</strong>
              <span>
                {mode.kind === "unsafe"
                  ? "忽略本次后台保护警告并完全退出。此确认只对本次有效。"
                  : "关闭主客户端。若后台保护尚未就绪，退出前会再次向你确认。"}
              </span>
            </span>
            {executing === "exit" ? <LoaderCircle className="window-close-option__spinner" size={22} /> : null}
          </button>
        </div>

        <footer className={`window-close-dialog__footer is-${mode.kind}`}>
          {mode.kind === "normal" ? (
            <label className="window-close-dialog__remember">
              <input
                type="checkbox"
                checked={remember}
                onChange={(event) => setRemember(event.target.checked)}
                disabled={Boolean(executing)}
              />
              <span className="window-close-dialog__checkbox" aria-hidden="true">
                <Check size={12} strokeWidth={2.6} />
              </span>
              <span>记住我的选择，下次直接执行</span>
            </label>
          ) : null}

          <button
            className="window-close-dialog__cancel"
            type="button"
            onClick={onCancel}
            disabled={Boolean(executing)}
          >
            {mode.kind === "unsafe" ? "取消退出" : "暂不退出"}
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
