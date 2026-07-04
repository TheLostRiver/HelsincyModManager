import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Search, X } from "lucide-react";
import type { GameSetupStartupNotice } from "./gameSetupTypes";
import { messageForError } from "./gameSetupViewModel";
import "./GameSetupFloatingNotice.css";

const AUTO_DISMISS_TIMEOUT_MS = 6000;

type GameSetupFloatingNoticeProps = {
  notice: GameSetupStartupNotice | null;
  isBusy: boolean;
  onRetry: () => Promise<void>;
  onManualSelect: (directory: string) => Promise<void>;
  onActionError: (message: string) => void;
  onDismiss: () => void;
};

export function GameSetupFloatingNotice({
  notice,
  isBusy,
  onRetry,
  onManualSelect,
  onActionError,
  onDismiss,
}: GameSetupFloatingNoticeProps) {
  const [isDismissPaused, setIsDismissPaused] = useState(false);

  useEffect(() => {
    if (!notice) {
      setIsDismissPaused(false);
    }
  }, [notice]);

  useEffect(() => {
    if (!notice || isDismissPaused) {
      return undefined;
    }

    const dismissTimer = window.setTimeout(() => onDismiss(), AUTO_DISMISS_TIMEOUT_MS);

    return () => window.clearTimeout(dismissTimer);
  }, [isDismissPaused, notice, onDismiss]);

  if (!notice) {
    return null;
  }

  async function handleRetry() {
    try {
      await onRetry();
    } catch {
      onActionError(messageForError("unknown"));
    }
  }

  async function handleManualSelect() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择《怪物猎人：世界 冰原》游戏目录",
      });

      if (typeof selected === "string") {
        await onManualSelect(selected);
      }
    } catch {
      onActionError(messageForError("unknown"));
    }
  }

  return (
    <aside
      className="game-setup-floating-notice"
      role="status"
      aria-live="polite"
      onPointerEnter={() => setIsDismissPaused(true)}
      onPointerLeave={() => setIsDismissPaused(false)}
      onFocus={() => setIsDismissPaused(true)}
      onBlur={() => setIsDismissPaused(false)}
    >
      <div className="game-setup-floating-notice__copy">
        <strong>{notice.title}</strong>
        <p>{notice.message}</p>
        <span>{notice.detail}</span>
      </div>

      <div className="game-setup-floating-notice__actions">
        <button type="button" className="primary-action" disabled={isBusy} onClick={() => void handleRetry()}>
          <Search size={16} />
          重新扫描
        </button>
        <button type="button" className="secondary-action" disabled={isBusy} onClick={() => void handleManualSelect()}>
          <FolderOpen size={16} />
          手动选择
        </button>
        <button
          type="button"
          className="game-setup-floating-notice__dismiss"
          aria-label="关闭本次游戏目录提示"
          onClick={onDismiss}
        >
          <X size={16} />
        </button>
      </div>
    </aside>
  );
}
