import { Gamepad2, Monitor, FlaskConical } from "lucide-react";

export const supportCards = [
  {
    label: "当前支持",
    value: "Monster Hunter: World - Iceborne",
    icon: Gamepad2,
    iconColor: "var(--color-accent)",
  },
  {
    label: "当前平台",
    value: "Windows",
    icon: Monitor,
    iconColor: "#10b981", // emerald-500
  },
  {
    label: "Linux / Steam Deck",
    value: "实验性支持预留",
    icon: FlaskConical,
    iconColor: "#a855f7", // purple-500
  },
] as const;

export const previewCards = [
  { label: "Mod 概览", shortWidth: "80px" },
  { label: "冲突状态", shortWidth: "72px" },
  { label: "前置检查", shortWidth: "76px" },
  { label: "最近备份", shortWidth: "70px" },
] as const;

export const setupSteps = [
  {
    title: "扫描 Steam 游戏库",
    meta: "检测已安装游戏和可用候选项。",
  },
  {
    title: "验证游戏目录",
    meta: "确认可执行文件、数据目录和写入权限。",
  },
  {
    title: "创建默认配置档案",
    meta: "在导入前准备一份干净的基线。",
  },
  {
    title: "开始导入模组",
    meta: "仅在目录和配置检查通过后启用。",
  },
] as const;

export const setupLogs = [
  { time: "09:42", message: "首次启动设置已打开" },
  { time: "09:42", message: "等待扫描 Steam 游戏库" },
  { time: "--:--", message: "尚未选择游戏目录", muted: true },
] as const;
