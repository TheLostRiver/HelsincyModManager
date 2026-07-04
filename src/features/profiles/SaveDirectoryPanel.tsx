import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Archive, CheckCircle2, FolderOpen, HardDrive, Search } from "lucide-react";
import { useState, type ReactNode } from "react";
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
  previewMode?: boolean;
  disabled?: boolean;
  onAutoDetect?: () => void;
  autoDetecting?: boolean;
  hasDiscoveryCandidates?: boolean;
};

export function SaveDirectoryPanel({
  gameId,
  profileId,
  settings,
  onSettingsChange,
  onDirectorySelected,
  previewMode = false,
  disabled = false,
  onAutoDetect,
  autoDetecting = false,
  hasDiscoveryCandidates = false,
}: SaveDirectoryPanelProps) {
  const [busyKind, setBusyKind] = useState<"saveDirectory" | "backupDirectory" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const chooseDirectory = async (kind: "saveDirectory" | "backupDirectory") => {
    if (disabled) return;
    setError(null);
    setBusyKind(kind);
    try {
      if (previewMode) {
        const directory =
          kind === "saveDirectory"
            ? "Steam/userdata/<steam-id>/582010/remote"
            : "HelsincyModManager/Backups/MHW";
        const selection: ProfileDirectorySelectionDto = {
          mode: "custom",
          status: "valid",
          pathLabel: directory,
          messages: ["预览环境已模拟校验通过"],
        };
        onDirectorySelected(kind, directory);
        onSettingsChange({ ...settings, [kind]: selection });
        return;
      }

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
    <section className={`profile-directory-console ${disabled ? "is-disabled" : ""}`} aria-labelledby="profile-save-directories-title">
      <div className="profile-directory-console__header panel-header-row">
        <div>
          <h2 id="profile-save-directories-title">存档目录</h2>
          <span>Save source and backup target</span>
        </div>
        <HardDrive size={18} aria-hidden="true" />
      </div>

      <div className="profile-directory-grid paths-setup-grid">
        <DirectoryCard
          icon={<FolderOpen size={20} />}
          label="游戏存档目录"
          selection={settings.saveDirectory}
          actionLabel={busyKind === "saveDirectory" ? "校验中..." : "选择路径"}
          disabled={disabled || busyKind !== null || autoDetecting}
          extraAction={
            onAutoDetect ? (
              <button
                type="button"
                className={`profile-action-button ${hasDiscoveryCandidates ? "is-primary" : ""}`}
                disabled={disabled || busyKind !== null || autoDetecting}
                onClick={onAutoDetect}
              >
                <Search size={14} />
                {autoDetecting ? "检测中..." : "自动检测"}
              </button>
            ) : null
          }
          onChoose={() => void chooseDirectory("saveDirectory")}
        />
        <DirectoryCard
          icon={<Archive size={20} />}
          label="备份存档目录"
          selection={settings.backupDirectory}
          actionLabel={busyKind === "backupDirectory" ? "校验中..." : "选择路径"}
          disabled={disabled || busyKind !== null}
          onChoose={() => void chooseDirectory("backupDirectory")}
        />
      </div>

      {error ? (
        <p className="profile-settings-alert" role="alert">
          <AlertTriangle size={14} />
          {error}
        </p>
      ) : null}
    </section>
  );
}

function DirectoryCard({
  icon,
  label,
  selection,
  actionLabel,
  disabled,
  extraAction,
  onChoose,
}: {
  icon: ReactNode;
  label: string;
  selection: ProfileDirectorySelectionDto;
  actionLabel: string;
  disabled: boolean;
  extraAction?: ReactNode;
  onChoose: () => void;
}) {
  const status = formatDirectoryStatus(selection);

  return (
    <div className={`profile-directory-card path-card status-${status.tone}`}>
      <div className="profile-directory-card__header path-card-top">
        <span className="profile-directory-card__icon path-icon-container" aria-hidden="true">
          {icon}
        </span>
        <div className="profile-directory-card__body path-card-details">
          <span>{label}</span>
          <strong className="profile-directory-card__path" title={status.label || "未选择"}>
            {status.label || "未选择"}
          </strong>
        </div>
      </div>

      <div className="profile-directory-card__footer path-card-footer">
        <span className={`profile-status-pill path-badge is-${status.tone}`}>
          {status.tone === "success" ? <CheckCircle2 size={13} /> : null}
          {selection.status}
        </span>
        {selection.messages.length > 0 ? (
          <small className="profile-directory-card__msg">{selection.messages[0]}</small>
        ) : (
          <small className="profile-directory-card__msg-placeholder">&nbsp;</small>
        )}
        <button
          type="button"
          className="profile-action-button is-primary"
          disabled={disabled}
          onClick={onChoose}
        >
          {actionLabel}
        </button>
        {extraAction}
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
