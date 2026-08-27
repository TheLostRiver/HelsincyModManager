import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Archive, CheckCircle2, FolderOpen, HardDrive, Search } from "lucide-react";
import { useState, type ReactNode } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import {
  openProfileDirectory,
  validateProfileBackupDirectory,
  validateProfileSaveDirectory,
} from "./profileSaveSettingsApi";
import type {
  ProfileDirectorySelectionDto,
  ProfileSaveSettingsDto,
} from "./profileSaveSettingsTypes";
import { formatDirectoryStatus } from "./profileViewModel";
import { createPreviewDirectorySelection } from "./profilesPreviewData";
import { saveDirectoryCopy, type SaveDirectoryCopy } from "./saveDirectoryCopy";

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
  const { locale } = useI18n();
  const copy = resolveCopy(saveDirectoryCopy, locale);
  const [busyKind, setBusyKind] = useState<"saveDirectory" | "backupDirectory" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const openFolder = async (kind: "save" | "backup") => {
    setError(null);
    try {
      await openProfileDirectory({ gameId, profileId, kind });
    } catch (err) {
      // 目录可能在配置之后被删除或替换成链接,后端会拒绝打开——这里给可恢复提示。
      setError(getPanelErrorMessage(err, copy.panel.openFolderFailed));
    }
  };

  const chooseDirectory = async (kind: "saveDirectory" | "backupDirectory") => {
    if (disabled) return;
    setError(null);
    setBusyKind(kind);
    try {
      if (previewMode) {
        const { directory, selection } = createPreviewDirectorySelection(kind);
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
      setError(getPanelErrorMessage(err, copy.panel.errorFallback));
    } finally {
      setBusyKind(null);
    }
  };

  return (
    <section
      className={`profile-directory-console ${disabled ? "is-disabled" : ""}`}
      aria-labelledby="profile-save-directories-title"
      data-tour-id="profiles.save-directories"
    >
      <div className="profile-directory-summary">
        <div className="profile-directory-summary__header">
          <span className="profile-directory-summary__icon" aria-hidden="true">
            <HardDrive size={15} />
          </span>
          <div>
            <h2 id="profile-save-directories-title">{copy.panel.title}</h2>
            <span>{copy.panel.subtitle}</span>
          </div>
        </div>

        <div className="profile-directory-summary__rows">
          <DirectoryRow
            icon={<FolderOpen size={15} />}
            label={copy.panel.saveRowLabel}
            selection={settings.saveDirectory}
            statusLabels={copy.directoryStatus}
            actionLabel={busyKind === "saveDirectory" ? copy.panel.validating : copy.panel.choose}
            disabled={disabled || busyKind !== null || autoDetecting}
            extraAction={
              onAutoDetect ? (
                <button
                  type="button"
                  className={`profile-directory-row__button ${hasDiscoveryCandidates ? "is-primary" : ""}`}
                  disabled={disabled || busyKind !== null || autoDetecting}
                  onClick={onAutoDetect}
                >
                  <Search size={13} />
                  {autoDetecting ? copy.panel.detecting : copy.panel.autoDetect}
                </button>
              ) : null
            }
            onChoose={() => void chooseDirectory("saveDirectory")}
            openFolderLabel={copy.panel.openFolder}
            openFolderDisabled={previewMode}
            canOpenDefaulted={false}
            onOpenFolder={() => void openFolder("save")}
          />
          <DirectoryRow
            icon={<Archive size={15} />}
            label={copy.panel.backupRowLabel}
            selection={settings.backupDirectory}
            statusLabels={copy.directoryStatus}
            actionLabel={busyKind === "backupDirectory" ? copy.panel.validating : copy.panel.choose}
            disabled={disabled || busyKind !== null}
            onChoose={() => void chooseDirectory("backupDirectory")}
            openFolderLabel={copy.panel.openFolder}
            openFolderDisabled={previewMode}
            canOpenDefaulted
            onOpenFolder={() => void openFolder("backup")}
          />
        </div>
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

function DirectoryRow({
  icon,
  label,
  selection,
  statusLabels,
  actionLabel,
  disabled,
  extraAction,
  onChoose,
  openFolderLabel,
  openFolderDisabled,
  canOpenDefaulted = false,
  onOpenFolder,
}: {
  icon: ReactNode;
  label: string;
  selection: ProfileDirectorySelectionDto;
  statusLabels: SaveDirectoryCopy["directoryStatus"];
  actionLabel: string;
  disabled: boolean;
  extraAction?: ReactNode;
  onChoose: () => void;
  openFolderLabel: string;
  openFolderDisabled: boolean;
  canOpenDefaulted?: boolean;
  onOpenFolder: () => void;
}) {
  const status = formatDirectoryStatus(selection, statusLabels);

  return (
    <div className={`profile-directory-row is-${status.tone}`}>
      <span className="profile-directory-row__icon" aria-hidden="true">
        {icon}
      </span>
      <div className="profile-directory-row__copy">
        <span>{label}</span>
        <strong className="profile-directory-row__path" title={status.label || statusLabels.unset}>
          {status.label || statusLabels.unset}
        </strong>
      </div>

      <div className="profile-directory-row__actions">
        <span className={`profile-status-pill profile-directory-row__status is-${status.tone}`}>
          {status.tone === "success" ? <CheckCircle2 size={13} /> : null}
          {selection.status}
        </span>
        {/* 只在目录确实可打开时渲染。前端 DTO 刻意不含真实路径,所以按状态判断:
            - unset:没有可打开的东西
            - invalid:打开必然失败
            - valid:后端直接打开已配置的自定义目录
            - defaulted:后端按 selection.mode 解析托管默认布局,目录缺失时按需
              补建(只补应用自有托管树)再打开,前端仍然不经手路径。托管默认只
            存在于备份目录,由调用处以 canOpenDefaulted 声明——存档行即使出现
            defaulted 也不渲染入口,不给玩家一个必报错的按钮。
            预览环境没有后端,打开又是无法造假的真实 OS 动作,按钮禁用而非隐藏:
            保留可见形态,也不用新增文案。 */}
        {selection.status === "valid" || (canOpenDefaulted && selection.status === "defaulted") ? (
          <button
            type="button"
            className="profile-directory-row__button"
            disabled={disabled || openFolderDisabled}
            onClick={onOpenFolder}
            title={openFolderLabel}
          >
            <FolderOpen size={13} />
            {openFolderLabel}
          </button>
        ) : null}
        <button
          type="button"
          className="profile-directory-row__button"
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

function getPanelErrorMessage(error: unknown, fallback: string) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = String((error as { message?: unknown }).message ?? "").trim();
    if (message) return message;
  }
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}
