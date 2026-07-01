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
  disabled?: boolean;
};

export function BackupPolicyPanel({
  settings,
  onScheduleChange,
  onRetentionChange,
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
                  maxAgeDays: event.target.value ? Math.max(1, Number(event.target.value) || 1) : null,
                })
              }
            />
          </label>
        </div>

        <button
          type="button"
          className="profile-action-button"
          disabled={disabled}
          onClick={() => onRetentionChange({ maxCount: 20, maxAgeDays: 30 })}
        >
          <RotateCcw size={14} />
          重置保留策略
        </button>
      </div>
    </section>
  );
}
