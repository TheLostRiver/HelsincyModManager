import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Archive, CheckCircle2, FolderOpen, HardDrive } from "lucide-react";
import { useState } from "react";
import {
  validateProfileBackupDirectory,
  validateProfileSaveDirectory,
} from "./profileSaveSettingsApi";
import type {
  ProfileDirectorySelectionDto,
  ProfileSaveSettingsDto,
} from "./profileSaveSettingsTypes";
import { formatDirectoryStatus } from "./profileViewModel";

type SaveDirectoryPanelProps = {
  gameId: string;
  profileId: string;
  settings: ProfileSaveSettingsDto;
  onSettingsChange: (settings: ProfileSaveSettingsDto) => void;
  onDirectorySelected: (kind: "saveDirectory" | "backupDirectory", directory: string) => void;
  disabled?: boolean;
};

export function SaveDirectoryPanel({
  gameId,
  profileId,
  settings,
  onSettingsChange,
  onDirectorySelected,
  disabled = false,
}: SaveDirectoryPanelProps) {
  const [busyKind, setBusyKind] = useState<"saveDirectory" | "backupDirectory" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const chooseDirectory = async (kind: "saveDirectory" | "backupDirectory") => {
    if (disabled) return;
    setError(null);
    setBusyKind(kind);
    try {
      const selected = await open({ directory: true, multiple: false });
      const directory = Array.isArray(selected) ? selected[0] : selected;
      if (!directory) return;

      const selection =
        kind === "saveDirectory"
          ? await validateProfileSaveDirectory({ gameId, profileId, directory })
          : await validateProfileBackupDirectory({ gameId, profileId, directory });
      onDirectorySelected(kind, directory);
      onSettingsChange({ ...settings, [kind]: selection });
    } catch (err) {
      setError(getPanelErrorMessage(err));
    } finally {
      setBusyKind(null);
    }
  };

  return (
    <section className={`profile-settings-panel ${disabled ? "is-disabled" : ""}`} aria-labelledby="profile-save-directories-title">
      <div className="profile-settings-panel__header">
        <div>
          <h2 id="profile-save-directories-title">存档目录</h2>
          <span>Save source and backup target</span>
        </div>
        <HardDrive size={18} aria-hidden="true" />
      </div>

      <DirectoryRow
        icon={<FolderOpen size={18} />}
        label="游戏存档目录"
        selection={settings.saveDirectory}
        actionLabel={busyKind === "saveDirectory" ? "校验中" : "选择"}
        disabled={disabled || busyKind !== null}
        onChoose={() => void chooseDirectory("saveDirectory")}
      />
      <DirectoryRow
        icon={<Archive size={18} />}
        label="备份存档目录"
        selection={settings.backupDirectory}
        actionLabel={busyKind === "backupDirectory" ? "校验中" : "选择"}
        disabled={disabled || busyKind !== null}
        onChoose={() => void chooseDirectory("backupDirectory")}
      />

      {error ? (
        <p className="profile-settings-alert" role="alert">
          <AlertTriangle size={14} />
          {error}
        </p>
      ) : null}
    </section>
  );
}

function DirectoryRow({
  icon,
  label,
  selection,
  actionLabel,
  disabled,
  onChoose,
}: {
  icon: React.ReactNode;
  label: string;
  selection: ProfileDirectorySelectionDto;
  actionLabel: string;
  disabled: boolean;
  onChoose: () => void;
}) {
  const status = formatDirectoryStatus(selection);

  return (
    <div className="profile-directory-row">
      <div className="profile-directory-row__main">
        <span className="profile-directory-row__icon" aria-hidden="true">
          {icon}
        </span>
        <div>
          <strong>{label}</strong>
          <span>{status.label}</span>
          {selection.messages.length > 0 ? <small>{selection.messages[0]}</small> : null}
        </div>
      </div>
      <div className="profile-directory-row__actions">
        <span className={`profile-status-pill is-${status.tone}`}>
          {status.tone === "success" ? <CheckCircle2 size={13} /> : null}
          {selection.status}
        </span>
        <button type="button" className="profile-action-button" disabled={disabled} onClick={onChoose}>
          {actionLabel}
        </button>
      </div>
    </div>
  );
}

function getPanelErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = String((error as { message?: unknown }).message ?? "").trim();
    if (message) return message;
  }
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "目录不可用";
}
