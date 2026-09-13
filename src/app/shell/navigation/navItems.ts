import {
  Archive,
  FileSearch,
  Info,
  LayoutDashboard,
  Puzzle,
  Settings,
  ShieldAlert,
  Tags,
  User,
} from "lucide-react";
import type { ComponentType } from "react";
import type { NavItemId } from "../../appShellCopy";

// 导航项只保留语义（id/图标/路由/布局位）；label 与禁用原因经 appShellCopy.nav 取。
export type NavItem = {
  id: NavItemId;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  route: string;
  placement?: "primary" | "utility";
};

export const navItems: NavItem[] = [
  { id: "dashboard", icon: LayoutDashboard, route: "/" },
  { id: "mods", icon: Puzzle, route: "/mods" },
  { id: "recovery", icon: ShieldAlert, route: "/recovery" },
  { id: "categories", icon: Tags, route: "/categories" },
  { id: "profiles", icon: User, route: "/profiles" },
  { id: "backups", icon: Archive, route: "/backups" },
  { id: "diagnostics", icon: FileSearch, route: "/diagnostics" },
  { id: "settings", icon: Settings, route: "/settings" },
  { id: "about", icon: Info, route: "/about", placement: "utility" },
];
