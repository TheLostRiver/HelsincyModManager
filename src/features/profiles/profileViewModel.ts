import type { BackupScheduleCopy, WeekdayIndex } from "./backupPolicyCopy";
import type { SaveDirectoryCopy } from "./saveDirectoryCopy";
import type {
  BackupCadence,
  ProfileBackupScheduleDto,
  ProfileDirectorySelectionDto,
} from "./profileSaveSettingsTypes";
import type { Profile } from "./profileTypes";

const defaultProfileId = "default";

export type ProfileMetrics = {
  totalCount: number;
  standbyCount: number;
  deletableCount: number;
};

export function isProfileDeletable(profile: Profile) {
  return profile.id !== defaultProfileId && !profile.isActive;
}

export function getProfileMetrics(profiles: Profile[]): ProfileMetrics {
  return profiles.reduce<ProfileMetrics>(
    (metrics, profile) => ({
      totalCount: metrics.totalCount + 1,
      standbyCount: metrics.standbyCount + (profile.isActive ? 0 : 1),
      deletableCount: metrics.deletableCount + (isProfileDeletable(profile) ? 1 : 0),
    }),
    { totalCount: 0, standbyCount: 0, deletableCount: 0 },
  );
}

export function formatDirectoryStatus(
  selection: ProfileDirectorySelectionDto,
  statusLabels: SaveDirectoryCopy["directoryStatus"],
) {
  switch (selection.status) {
    case "valid":
      return { label: selection.pathLabel ?? statusLabels.valid, tone: "success" as const };
    case "defaulted":
      return { label: selection.pathLabel ?? statusLabels.defaulted, tone: "neutral" as const };
    case "invalid":
      return { label: selection.pathLabel ?? statusLabels.invalid, tone: "warning" as const };
    case "unset":
      return { label: statusLabels.unset, tone: "warning" as const };
  }
}

export function formatBackupSchedule(schedule: ProfileBackupScheduleDto, copy: BackupScheduleCopy) {
  if (schedule.cadence === "manual") {
    return copy.manual;
  }

  const hour = String(schedule.hour ?? 0).padStart(2, "0");
  const minute = String(schedule.minute ?? 0).padStart(2, "0");
  const time = `${hour}:${minute}`;

  if (schedule.cadence === "daily") {
    return copy.daily(time);
  }

  if (schedule.weekdays.length === 0) return copy.weeklyEvery(time);
  if (schedule.weekdays.length === 7) return copy.weeklyAllDays(time);
  return copy.weeklyOn(formatWeekdays(schedule.weekdays, copy), time);
}

export function defaultSchedule(cadence: BackupCadence): ProfileBackupScheduleDto {
  if (cadence === "manual") {
    return { cadence, hour: null, minute: null, weekdays: [] };
  }

  if (cadence === "daily") {
    return { cadence, hour: 3, minute: 0, weekdays: [] };
  }

  return { cadence, hour: 3, minute: 0, weekdays: [1] };
}

// 周一为首的语义排序，与文案无关。
export const weekdayDisplayOrder: ReadonlyMap<number, number> = new Map([
  [1, 0],
  [2, 1],
  [3, 2],
  [4, 3],
  [5, 4],
  [6, 5],
  [0, 6],
]);

function formatWeekdays(days: number[], copy: BackupScheduleCopy) {
  return [...days]
    .sort((a, b) => (weekdayDisplayOrder.get(a) ?? 0) - (weekdayDisplayOrder.get(b) ?? 0))
    .map((day) => copy.weekdayLong[day as WeekdayIndex])
    .filter(Boolean)
    .join(copy.weekdayJoin);
}
