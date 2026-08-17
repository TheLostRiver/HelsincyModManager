import {
  Archive,
  Crosshair,
  FileSearch,
  Gamepad2,
  LayoutDashboard,
  ListChecks,
  Puzzle,
  Settings,
  ShieldAlert,
  Tags,
  User,
} from "lucide-react";
import type { ComponentType } from "react";

export type NavItem = {
  id: string;
  label: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  route: string;
  disabledReason?: string;
};

export const navItems: NavItem[] = [
  { id: "dashboard", label: "工作台", icon: LayoutDashboard, route: "/" },
  { id: "mods", label: "Mod 管理", icon: Puzzle, route: "/mods" },
  { id: "recovery", label: "恢复中心", icon: ShieldAlert, route: "/recovery" },
  { id: "categories", label: "分类 / 标签", icon: Tags, route: "/categories", disabledReason: "导入 Mod 后启用" },
  { id: "profiles", label: "存档备份", icon: User, route: "/profiles" },
  { id: "replacements", label: "替换目标", icon: Crosshair, route: "/replacements", disabledReason: "替换目标 catalog 接入后启用" },
  { id: "backups", label: "备份整理", icon: Archive, route: "/backups" },
  { id: "games", label: "游戏管理", icon: Gamepad2, route: "/games", disabledReason: "游戏管理页面尚未接入" },
  { id: "tasks", label: "任务队列", icon: ListChecks, route: "/tasks", disabledReason: "任务队列页面尚未接入" },
  { id: "diagnostics", label: "日志 / 诊断", icon: FileSearch, route: "/diagnostics" },
  { id: "settings", label: "设置", icon: Settings, route: "/settings" },
];
