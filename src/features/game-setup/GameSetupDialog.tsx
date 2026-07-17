import { FolderOpen, Search } from "lucide-react";
import { useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Dialog } from "../../shared/feedback";
import type { GameSetupStartupNotice } from "./gameSetupTypes";
import { messageForError } from "./gameSetupViewModel";
import "./GameSetupDialog.css";

type GameSetupDialogProps = {
  notice: GameSetupStartupNotice | null;
  isBusy: boolean;
  onRetry: () => Promise<void>;
  onManualSelect: (directory: string) => Promise<void>;
  onActionError: (message: string) => void;
  onDismiss: () => void;
};

export function GameSetupDialog({
  notice,
  isBusy,
  onRetry,
  onManualSelect,
  onActionError,
  onDismiss,
}: GameSetupDialogProps) {
  const retryButtonRef = useRef<HTMLButtonElement | null>(null);

  if (!notice) {
    return null;
  }

  const hasAdditionalDetail = notice.detail.trim() !== notice.message.trim();

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
    <Dialog
      open
      title={notice.title}
      description={notice.message}
      icon={<FolderOpen size={19} />}
      onClose={onDismiss}
      closeLabel="关闭游戏目录设置"
      closeOnEscape={!isBusy}
      closeOnBackdrop={!isBusy}
      busy={isBusy}
      initialFocusRef={retryButtonRef}
      footer={
        <div className="game-setup-dialog__actions">
          <button
            ref={retryButtonRef}
            type="button"
            className="primary-action"
            disabled={isBusy}
            onClick={() => void handleRetry()}
          >
            <Search size={16} />
            重新扫描
          </button>
          <button
            type="button"
            className="secondary-action"
            disabled={isBusy}
            onClick={() => void handleManualSelect()}
          >
            <FolderOpen size={16} />
            手动选择目录
          </button>
        </div>
      }
    >
      {hasAdditionalDetail || isBusy ? (
        <div className="game-setup-dialog__content">
          {hasAdditionalDetail ? <p>{notice.detail}</p> : null}
          {isBusy ? (
            <p className="game-setup-dialog__progress" role="status" aria-live="polite">
              正在检查游戏目录…
            </p>
          ) : null}
        </div>
      ) : null}
    </Dialog>
  );
}
