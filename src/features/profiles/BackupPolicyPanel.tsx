import { RotateCcw, ShieldCheck } from "lucide-react";
import { BackupSchedulePicker } from "./BackupSchedulePicker";
import type {
  ProfileBackupRetentionDto,
  ProfileBackupScheduleDto,
  ProfileSaveSettingsDto,
} from "./profileSaveSettingsTypes";

type BackupPolicyPanelProps = {
  settings: ProfileSaveSettingsDto;
  onScheduleChange: (schedule: ProfileBackupScheduleDto) => void;
  onRetentionChange: (retention: ProfileBackupRetentionDto) => void;
  onPreRestoreBackupEnabledChange: (enabled: boolean) => void;
  disabled?: boolean;
};

export function BackupPolicyPanel({
  settings,
  onScheduleChange,
  onRetentionChange,
  onPreRestoreBackupEnabledChange,
  disabled = false,
}: BackupPolicyPanelProps) {
  return (
    <section className={`profile-policy-card ${disabled ? "is-disabled" : ""}`} aria-labelledby="profile-backup-policy-title">
      <div className="profile-policy-card__header panel-header-row">
        <div>
          <h2 id="profile-backup-policy-title">自动备份</h2>
          <span>Schedule and retention</span>
        </div>
        <ShieldCheck size={18} aria-hidden="true" />
      </div>

      <div className="profile-policy-body">
        <BackupSchedulePicker schedule={settings.schedule} onChange={onScheduleChange} disabled={disabled} />

        <div className="profile-retention-grid">
          <label className="profile-field">
            <span>保留数量</span>
            <input
              type="number"
              min={1}
              max={999}
              value={settings.retention.maxCount}
              disabled={disabled}
              onChange={(event) =>
                onRetentionChange({
                  ...settings.retention,
                  maxCount: Math.max(1, Number(event.target.value) || 1),
                })
              }
            />
          </label>
          <label className="profile-field">
            <span>保留天数</span>
            <input
              type="number"
              min={1}
              max={3650}
              value={settings.retention.maxAgeDays ?? ""}
              disabled={disabled}
              onChange={(event) =>
                onRetentionChange({
                  ...settings.retention,
                  maxAgeDays: event.target.value
                    ? Math.min(3650, Math.max(1, Math.floor(Number(event.target.value) || 1)))
                    : null,
                })
              }
            />
          </label>
          <label className="profile-field">
            <span>空间上限（MiB）</span>
            <input
              type="number"
              min={16}
              max={1048576}
              value={settings.retention.maxTotalBytes === null ? "" : Math.round(settings.retention.maxTotalBytes / (1024 * 1024))}
              step={1}
              disabled={disabled}
              onChange={(event) =>
                onRetentionChange({
                  ...settings.retention,
                  maxTotalBytes: event.target.value
                    ? Math.min(1_048_576, Math.max(16, Math.floor(Number(event.target.value) || 16))) * 1024 * 1024
                    : null,
                })
              }
            />
          </label>
        </div>

        <div className="profile-pre-restore-setting">
          <div>
            <strong>恢复前安全备份</strong>
            <span>恢复存档前先创建独立保护点，默认开启。</span>
          </div>
          <input
            type="checkbox"
            checked={settings.preRestoreBackupEnabled}
            disabled={disabled}
            aria-label="恢复前自动备份"
            onChange={(event) => onPreRestoreBackupEnabledChange(event.target.checked)}
          />
        </div>

        <button
          type="button"
          className="profile-action-button"
          disabled={disabled}
          onClick={() => onRetentionChange({ maxCount: 20, maxAgeDays: 30, maxTotalBytes: null })}
        >
          <RotateCcw size={14} />
          重置保留策略
        </button>
      </div>
    </section>
  );
}
