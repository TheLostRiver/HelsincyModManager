import type {
  BackupCadence,
  ProfileBackupScheduleDto,
  ProfileDirectorySelectionDto,
} from "./profileSaveSettingsTypes";
import type { Profile } from "./profileTypes";

export type ProfileMetrics = {
  totalCount: number;
  standbyCount: number;
  deletableCount: number;
};

export function getProfileMetrics(profiles: Profile[]): ProfileMetrics {
  return profiles.reduce<ProfileMetrics>(
    (metrics, profile) => ({
      totalCount: metrics.totalCount + 1,
      standbyCount: metrics.standbyCount + (profile.isActive ? 0 : 1),
      deletableCount:
        metrics.deletableCount + (profile.id !== "default" && !profile.isActive ? 1 : 0),
    }),
    { totalCount: 0, standbyCount: 0, deletableCount: 0 },
  );
}

export function formatDirectoryStatus(selection: ProfileDirectorySelectionDto) {
  switch (selection.status) {
    case "valid":
      return { label: selection.pathLabel ?? "已配置", tone: "success" as const };
    case "defaulted":
      return { label: selection.pathLabel ?? "默认目录", tone: "neutral" as const };
    case "invalid":
      return { label: selection.pathLabel ?? "目录不可用", tone: "warning" as const };
    case "unset":
      return { label: "未选择", tone: "warning" as const };
  }
}

export function formatBackupSchedule(schedule: ProfileBackupScheduleDto) {
  if (schedule.cadence === "manual") {
    return "仅手动";
  }

  const hour = String(schedule.hour ?? 0).padStart(2, "0");
  const minute = String(schedule.minute ?? 0).padStart(2, "0");

  if (schedule.cadence === "daily") {
    return `每日 ${hour}:${minute}`;
  }

  return `${formatWeekdays(schedule.weekdays)} ${hour}:${minute}`;
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

function formatWeekdays(days: number[]) {
  if (days.length === 0) return "每周";
  if (days.length === 7) return "每天";

  const labels = new Map([
    [1, "星期一"],
    [2, "星期二"],
    [3, "星期三"],
    [4, "星期四"],
    [5, "星期五"],
    [6, "星期六"],
    [0, "星期日"],
  ]);
  const order = new Map([
    [1, 0],
    [2, 1],
    [3, 2],
    [4, 3],
    [5, 4],
    [6, 5],
    [0, 6],
  ]);

  return `每周${[...days]
    .sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0))
    .map((day) => labels.get(day))
    .filter(Boolean)
    .join("、")}`;
}
