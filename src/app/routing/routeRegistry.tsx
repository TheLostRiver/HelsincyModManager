import { DashboardPage } from "../../features/dashboard/DashboardPage";
import { ModLibraryPage } from "../../features/mods/ModLibraryPage";
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
] satisfies AppRoute[];

export const enabledRouteIds = new Set(appRoutes.map((route) => route.id));
