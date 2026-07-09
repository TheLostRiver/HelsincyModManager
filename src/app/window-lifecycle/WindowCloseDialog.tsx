import { Check, LoaderCircle, Minimize2, Power, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { WindowClosePreference } from "./windowClosePreference";
import "./WindowCloseDialog.css";

type WindowCloseDialogProps = {
  open: boolean;
  errorMessage: string | null;
  onCancel: () => void;
  onConfirm: (action: WindowClosePreference, remember: boolean) => Promise<void>;
};

type ExecutingAction = "tray" | "exit" | null;

const EXECUTION_FEEDBACK_DELAY_MS = 360;

export function WindowCloseDialog({ open, errorMessage, onCancel, onConfirm }: WindowCloseDialogProps) {
  const [remember, setRemember] = useState(false);
  const [executing, setExecuting] = useState<ExecutingAction>(null);
  const [successText, setSuccessText] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    setRemember(false);
    setExecuting(null);
    setSuccessText(null);
    window.setTimeout(() => dialogRef.current?.focus(), 0);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !executing) onCancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [executing, onCancel, open]);

  if (!open) return null;

  const execute = async (action: "tray" | "exit") => {
    setExecuting(action);
    setSuccessText(action === "tray" ? "已收起至系统托盘" : "正在退出应用");
    try {
      await new Promise((resolve) => window.setTimeout(resolve, EXECUTION_FEEDBACK_DELAY_MS));
      await onConfirm(action, remember);
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
        className="window-close-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="window-close-title"
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
          <h2 id="window-close-title">准备退出 Helsincy？</h2>
          <p>请选择关闭主窗口时的操作。你也可以在设置里随时改回每次询问。</p>
        </header>

        {errorMessage ? <p className="window-close-dialog__error">{errorMessage}</p> : null}

        <div className="window-close-dialog__options">
          <button
            className="window-close-option is-tray"
            type="button"
            onClick={() => void execute("tray")}
            disabled={Boolean(executing)}
          >
            <span className="window-close-option__icon" aria-hidden="true">
              <Minimize2 size={24} strokeWidth={2.15} />
            </span>
            <span className="window-close-option__copy">
              <strong>收起至系统托盘</strong>
              <span>应用将在后台持续运行，自动备份仍会在客户端运行期间检查。</span>
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
              <strong>完全退出应用程序</strong>
              <span>关闭主客户端。后台守护落地前，自动备份不会继续检查。</span>
            </span>
            {executing === "exit" ? <LoaderCircle className="window-close-option__spinner" size={22} /> : null}
          </button>
        </div>

        <footer className="window-close-dialog__footer">
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

          <button
            className="window-close-dialog__cancel"
            type="button"
            onClick={onCancel}
            disabled={Boolean(executing)}
          >
            暂不退出
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
