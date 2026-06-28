import { DashboardPage } from "../../features/dashboard/DashboardPage";
import { RecoveryCenterPage } from "../../features/install-recovery/RecoveryCenterPage";
import { ModLibraryPage } from "../../features/mods/ModLibraryPage";
import { SettingsPage } from "../../features/settings/SettingsPage";
import { CategoryPage } from "../../features/categories/CategoryPage";
import type { AppRoute } from "./routeTypes";

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
    id: "categories",
    path: "/categories",
    element: CategoryPage,
  },
  {
    id: "settings",
    path: "/settings",
    element: SettingsPage,
  },
] satisfies AppRoute[];

export const enabledRouteIds = new Set(appRoutes.map((route) => route.id));
