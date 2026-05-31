import {
  Archive,
  Crosshair,
  FileSearch,
  Gamepad2,
  LayoutDashboard,
  ListChecks,
  Puzzle,
  Settings,
  Tags,
  User,
} from "lucide-react";
import type { ComponentType } from "react";

export type NavItemState = "active" | "disabled";

export type NavItem = {
  id: string;
  label: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  route: string;
  state?: NavItemState;
  disabledReason?: string;
};

export const navItems: NavItem[] = [
  { id: "dashboard", label: "工作台", icon: LayoutDashboard, route: "/", state: "active" },
  { id: "mods", label: "Mod 管理", icon: Puzzle, route: "/mods", state: "disabled", disabledReason: "完成游戏目录设置后启用" },
  { id: "categories", label: "分类 / 标签", icon: Tags, route: "/categories", state: "disabled", disabledReason: "导入 Mod 后启用" },
  { id: "profiles", label: "配置档", icon: User, route: "/profiles", state: "disabled", disabledReason: "创建默认配置档后启用" },
  { id: "replacements", label: "替换目标", icon: Crosshair, route: "/replacements", state: "disabled", disabledReason: "替换目标 catalog 接入后启用" },
  { id: "backups", label: "存档备份", icon: Archive, route: "/backups", state: "disabled", disabledReason: "存档路径规则接入后启用" },
  { id: "games", label: "游戏管理", icon: Gamepad2, route: "/games" },
  { id: "tasks", label: "任务队列", icon: ListChecks, route: "/tasks" },
  { id: "diagnostics", label: "日志 / 诊断", icon: FileSearch, route: "/diagnostics" },
  { id: "settings", label: "设置", icon: Settings, route: "/settings" },
];
