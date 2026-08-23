import { RotateCcw, ShieldCheck } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { BackupSchedulePicker } from "./BackupSchedulePicker";
import { backupPolicyCopy } from "./backupPolicyCopy";
import { DEFAULT_PROFILE_BACKUP_RETENTION } from "./profileSaveSettingsDefaults";
import type {
  ProfileBackupRetentionDto,
  ProfileBackupScheduleDto,
  ProfileSaveSettingsDto,
} from "./profileSaveSettingsTypes";

const MAX_RETENTION_COUNT = 999;
const MAX_RETENTION_AGE_DAYS = 3650;
const MIN_RETENTION_TOTAL_MIB = 16;
const MAX_RETENTION_TOTAL_MIB = 1_048_576;
const MEBIBYTE = 1024 * 1024;

function clampInteger(rawValue: string, minimum: number, maximum: number, fallback: number) {
  const parsed = Math.floor(Number(rawValue));
  return Number.isFinite(parsed) ? Math.min(maximum, Math.max(minimum, parsed)) : fallback;
}

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
  const { locale } = useI18n();
  const copy = resolveCopy(backupPolicyCopy, locale).panel;

  return (
    <section
      className={`profile-policy-card ${disabled ? "is-disabled" : ""}`}
      aria-labelledby="profile-backup-policy-title"
      data-tour-id="profiles.backup-policy"
    >
      <div className="profile-policy-card__header panel-header-row">
        <div>
          <h2 id="profile-backup-policy-title">{copy.title}</h2>
          <span>Schedule and retention</span>
        </div>
        <ShieldCheck size={18} aria-hidden="true" />
      </div>

      <div className="profile-policy-body">
        <BackupSchedulePicker schedule={settings.schedule} onChange={onScheduleChange} disabled={disabled} />

        <div className="profile-retention-grid">
          <label className="profile-field">
            <span>{copy.retentionCount}</span>
            <input
              type="number"
              min={0}
              max={MAX_RETENTION_COUNT}
              value={settings.retention.maxCount}
              disabled={disabled}
              aria-describedby="profile-retention-unlimited-note"
              onChange={(event) =>
                onRetentionChange({
                  ...settings.retention,
                  maxCount: clampInteger(event.target.value, 0, MAX_RETENTION_COUNT, 0),
                })
              }
            />
          </label>
          <label className="profile-field">
            <span>{copy.retentionDays}</span>
            <input
              type="number"
              min={0}
              max={MAX_RETENTION_AGE_DAYS}
              value={settings.retention.maxAgeDays ?? 0}
              disabled={disabled}
              aria-describedby="profile-retention-unlimited-note"
              onChange={(event) => {
                const maxAgeDays = clampInteger(event.target.value, 0, MAX_RETENTION_AGE_DAYS, 0);
                onRetentionChange({
                  ...settings.retention,
                  maxAgeDays: maxAgeDays === 0 ? null : maxAgeDays,
                });
              }}
            />
          </label>
          <label className="profile-field">
            <span>{copy.retentionSpace}</span>
            <input
              type="number"
              min={0}
              max={MAX_RETENTION_TOTAL_MIB}
              value={settings.retention.maxTotalBytes === null ? 0 : Math.round(settings.retention.maxTotalBytes / MEBIBYTE)}
              step={1}
              disabled={disabled}
              aria-describedby="profile-retention-unlimited-note"
              onChange={(event) => {
                const maxTotalMiB = clampInteger(event.target.value, 0, MAX_RETENTION_TOTAL_MIB, 0);
                onRetentionChange({
                  ...settings.retention,
                  maxTotalBytes:
                    maxTotalMiB === 0 ? null : Math.max(MIN_RETENTION_TOTAL_MIB, maxTotalMiB) * MEBIBYTE,
                });
              }}
            />
          </label>
        </div>
        <p id="profile-retention-unlimited-note" className="profile-retention-note">
          {copy.unlimitedNote}
        </p>

        <div className="profile-pre-restore-setting">
          <div>
            <strong>{copy.preRestoreTitle}</strong>
            <span>{copy.preRestoreHint}</span>
          </div>
          <input
            type="checkbox"
            checked={settings.preRestoreBackupEnabled}
            disabled={disabled}
            aria-label={copy.preRestoreAria}
            onChange={(event) => onPreRestoreBackupEnabledChange(event.target.checked)}
          />
        </div>

        <button
          type="button"
          className="profile-action-button"
          disabled={disabled}
          onClick={() => onRetentionChange({ ...DEFAULT_PROFILE_BACKUP_RETENTION })}
        >
          <RotateCcw size={14} />
          {copy.resetRetention}
        </button>
      </div>
    </section>
  );
}
