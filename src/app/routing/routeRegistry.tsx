import { DashboardPage } from "../../features/dashboard/DashboardPage";
import { RecoveryCenterPage } from "../../features/install-recovery/RecoveryCenterPage";
import { ModLibraryPage } from "../../features/mods/ModLibraryPage";
import { SettingsPage } from "../../features/settings/SettingsPage";
import { CategoryPage } from "../../features/categories/CategoryPage";
import { ProfilePage } from "../../features/profiles/ProfilePage";
import type { AppRoute } from "./routeTypes";
import { DiagnosticsPage } from "../../features/diagnostics/DiagnosticsPage";
import { BackupCenterPage } from "../../features/backups/BackupCenterPage";

export const appRoutes = [
  {
    id: "dashboard",
    path: "/",
    element: DashboardPage,
  },
  {
    id: "mods",
    path: "/mods",
    element: ModLibraryPage,
  },
  {
    id: "recovery",
    path: "/recovery",
    element: RecoveryCenterPage,
  },
  {
    id: "diagnostics",
    path: "/diagnostics",
    element: DiagnosticsPage,
  },
  {
    id: "categories",
    path: "/categories",
    element: CategoryPage,
  },
  {
    id: "profiles",
    path: "/profiles",
    element: ProfilePage,
  },
  {
    id: "backups",
    path: "/backups",
    element: BackupCenterPage,
  },
  {
    id: "settings",
    path: "/settings",
    element: SettingsPage,
  },
] satisfies AppRoute[];

export const enabledRouteIds = new Set(appRoutes.map((route) => route.id));
