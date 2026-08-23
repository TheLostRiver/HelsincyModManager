import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Search } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { gameSetupCopy } from "./gameSetupCopy";
import { messageForError } from "./gameSetupViewModel";

type GameDirectoryActionsProps = {
  isBusy: boolean;
  onDirectorySelected: (directory: string) => Promise<void>;
  onActionError: (message: string) => void;
  onScanSteam: () => Promise<void>;
};

export function GameDirectoryActions({
  isBusy,
  onDirectorySelected,
  onActionError,
  onScanSteam,
}: GameDirectoryActionsProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(gameSetupCopy, locale);

  async function handleManualSelect() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: copy.actions.dialogTitle,
      });

      if (typeof selected === "string") {
        await onDirectorySelected(selected);
      }
    } catch {
      onActionError(messageForError("unknown", copy.errors));
    }
  }

  async function handleSteamScan() {
    try {
      await onScanSteam();
    } catch {
      onActionError(messageForError("unknown", copy.errors));
    }
  }

  return (
    <div className="setup-actions" data-tour-id="dashboard.directory-actions">
      <button
        type="button"
        className="primary-action"
        disabled={isBusy}
        data-tour-id="dashboard.steam-scan"
        onClick={() => void handleSteamScan()}
      >
        <Search size={16} />
        {copy.actions.scanSteam}
      </button>
      <button
        type="button"
        className="secondary-action"
        disabled={isBusy}
        data-tour-id="dashboard.manual-directory"
        onClick={() => void handleManualSelect()}
      >
        <FolderOpen size={16} />
        {copy.actions.manualSelect}
      </button>
    </div>
  );
}
