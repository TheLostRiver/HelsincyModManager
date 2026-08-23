import { Gamepad2, Monitor, FlaskConical } from "lucide-react";

// 支持卡与模块预览卡的语义结构（图标、颜色、布局宽度、专有名词值）。
// 可翻译标签经 labelKey/valueKey 指向 dashboardCopy.supportCards / modulePreview.cards；
// 「Monster Hunter: World - Iceborne」「Windows」「Linux / Steam Deck」为专有名词保持原文。

export const supportCards = [
  {
    id: "current-game",
    labelKey: "currentGame",
    value: "Monster Hunter: World - Iceborne",
    icon: Gamepad2,
    iconColor: "var(--color-accent)",
  },
  {
    id: "current-platform",
    labelKey: "currentPlatform",
    value: "Windows",
    icon: Monitor,
    iconColor: "#10b981", // emerald-500
  },
  {
    id: "linux-steam-deck",
    label: "Linux / Steam Deck",
    valueKey: "experimentalReserved",
    icon: FlaskConical,
    iconColor: "#a855f7", // purple-500
  },
] as const;

export const previewCards = [
  { labelKey: "modOverview", shortWidth: "80px" },
  { labelKey: "conflictStatus", shortWidth: "72px" },
  { labelKey: "prerequisiteCheck", shortWidth: "76px" },
  { labelKey: "recentBackup", shortWidth: "70px" },
] as const;
