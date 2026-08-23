import type { LocaleDictionary } from "../../shared/i18n";

// 自动备份策略（BackupPolicyPanel + BackupSchedulePicker + 计划摘要格式化）的
// 全部用户可见文案。星期缩写不再从长标签截取尾字，各语言显式给出。

export type WeekdayIndex = 0 | 1 | 2 | 3 | 4 | 5 | 6;

export type BackupScheduleCopy = {
  manual: string;
  daily: (time: string) => string;
  weeklyEvery: (time: string) => string;
  weeklyAllDays: (time: string) => string;
  weeklyOn: (days: string, time: string) => string;
  weekdayLong: Record<WeekdayIndex, string>;
  weekdayJoin: string;
};

export type BackupPolicyCopy = {
  schedule: BackupScheduleCopy;
  picker: {
    manualChip: string;
    dailyChip: string;
    weeklyChip: string;
    weekdayAbbr: Record<WeekdayIndex, string>;
    abbrJoin: string;
    weeklyAbbr: (days: string) => string;
    weeklyAbbrEmpty: string;
    timeDialogAria: string;
    weekdayGroupAria: string;
    done: string;
    hourUnit: string;
    minuteUnit: string;
    decreaseAria: (unit: string) => string;
    increaseAria: (unit: string) => string;
  };
  panel: {
    title: string;
    retentionCount: string;
    retentionDays: string;
    retentionSpace: string;
    unlimitedNote: string;
    preRestoreTitle: string;
    preRestoreHint: string;
    preRestoreAria: string;
    resetRetention: string;
  };
};

export const backupPolicyCopy = {
  zh_cn: {
    schedule: {
      manual: "仅手动",
      daily: (time: string) => `每日 ${time}`,
      weeklyEvery: (time: string) => `每周 ${time}`,
      weeklyAllDays: (time: string) => `每天 ${time}`,
      weeklyOn: (days: string, time: string) => `每周${days} ${time}`,
      weekdayLong: { 1: "星期一", 2: "星期二", 3: "星期三", 4: "星期四", 5: "星期五", 6: "星期六", 0: "星期日" },
      weekdayJoin: "、",
    },
    picker: {
      manualChip: "手动",
      dailyChip: "每日备份",
      weeklyChip: "每周备份",
      weekdayAbbr: { 1: "一", 2: "二", 3: "三", 4: "四", 5: "五", 6: "六", 0: "日" },
      abbrJoin: ",",
      weeklyAbbr: (days: string) => `周${days}`,
      weeklyAbbrEmpty: "周日",
      timeDialogAria: "自动备份时间",
      weekdayGroupAria: "每周日期",
      done: "确定",
      hourUnit: "时",
      minuteUnit: "分",
      decreaseAria: (unit: string) => `减少${unit}`,
      increaseAria: (unit: string) => `增加${unit}`,
    },
    panel: {
      title: "自动备份",
      retentionCount: "保留数量",
      retentionDays: "保留天数",
      retentionSpace: "空间上限（MiB）",
      unlimitedNote: "数量、天数和空间上限：0 = 不限制",
      preRestoreTitle: "恢复前安全备份",
      preRestoreHint: "恢复存档前先创建独立保护点，默认开启。",
      preRestoreAria: "恢复前自动备份",
      resetRetention: "重置保留策略",
    },
  },
  en: {
    schedule: {
      manual: "Manual only",
      daily: (time: string) => `Daily at ${time}`,
      weeklyEvery: (time: string) => `Weekly at ${time}`,
      weeklyAllDays: (time: string) => `Every day at ${time}`,
      weeklyOn: (days: string, time: string) => `Weekly on ${days} at ${time}`,
      weekdayLong: { 1: "Monday", 2: "Tuesday", 3: "Wednesday", 4: "Thursday", 5: "Friday", 6: "Saturday", 0: "Sunday" },
      weekdayJoin: ", ",
    },
    picker: {
      manualChip: "Manual",
      dailyChip: "Daily backup",
      weeklyChip: "Weekly backup",
      weekdayAbbr: { 1: "Mon", 2: "Tue", 3: "Wed", 4: "Thu", 5: "Fri", 6: "Sat", 0: "Sun" },
      abbrJoin: ", ",
      weeklyAbbr: (days: string) => days,
      weeklyAbbrEmpty: "Sun",
      timeDialogAria: "Auto backup time",
      weekdayGroupAria: "Weekly days",
      done: "Done",
      hourUnit: "h",
      minuteUnit: "min",
      decreaseAria: (unit: string) => `Decrease ${unit}`,
      increaseAria: (unit: string) => `Increase ${unit}`,
    },
    panel: {
      title: "Auto backup",
      retentionCount: "Retained count",
      retentionDays: "Retained days",
      retentionSpace: "Space limit (MiB)",
      unlimitedNote: "Count, days, and space limit: 0 = unlimited",
      preRestoreTitle: "Pre-restore safety backup",
      preRestoreHint: "Creates an independent protection point before restoring save data. On by default.",
      preRestoreAria: "Automatic backup before restore",
      resetRetention: "Reset retention policy",
    },
  },
  ja: {
    schedule: {
      manual: "手動のみ",
      daily: (time: string) => `毎日 ${time}`,
      weeklyEvery: (time: string) => `毎週 ${time}`,
      weeklyAllDays: (time: string) => `毎日 ${time}`,
      weeklyOn: (days: string, time: string) => `毎週${days} ${time}`,
      weekdayLong: { 1: "月曜日", 2: "火曜日", 3: "水曜日", 4: "木曜日", 5: "金曜日", 6: "土曜日", 0: "日曜日" },
      weekdayJoin: "・",
    },
    picker: {
      manualChip: "手動",
      dailyChip: "毎日バックアップ",
      weeklyChip: "毎週バックアップ",
      weekdayAbbr: { 1: "月", 2: "火", 3: "水", 4: "木", 5: "金", 6: "土", 0: "日" },
      abbrJoin: "・",
      weeklyAbbr: (days: string) => `週${days}`,
      weeklyAbbrEmpty: "日曜",
      timeDialogAria: "自動バックアップ時刻",
      weekdayGroupAria: "毎週の曜日",
      done: "決定",
      hourUnit: "時",
      minuteUnit: "分",
      decreaseAria: (unit: string) => `${unit}を減らす`,
      increaseAria: (unit: string) => `${unit}を増やす`,
    },
    panel: {
      title: "自動バックアップ",
      retentionCount: "保持数",
      retentionDays: "保持日数",
      retentionSpace: "容量上限（MiB）",
      unlimitedNote: "数・日数・容量上限：0 = 無制限",
      preRestoreTitle: "復元前セーフティバックアップ",
      preRestoreHint: "セーブデータを復元する前に独立した保護ポイントを作成します。既定で有効。",
      preRestoreAria: "復元前の自動バックアップ",
      resetRetention: "保持ポリシーをリセット",
    },
  },
} satisfies LocaleDictionary<BackupPolicyCopy>;
