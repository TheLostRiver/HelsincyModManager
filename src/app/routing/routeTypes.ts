import type { ComponentType } from "react";

export type AppRouteId =
  | "dashboard"
  | "mods"
  | "recovery"
  | "diagnostics"
  | "categories"
  | "profiles"
  | "backups"
  | "settings"
  | "about";

export type AppRoute = {
  id: AppRouteId;
  path: string;
  element: ComponentType;
};

export type RouteLike = {
  id: string;
  path: string;
};

export type NavigationItemLike = {
  id: string;
  route: string;
};

export type NavigationStateItem<TItem extends NavigationItemLike> = TItem & {
  isActive: boolean;
  isDisabled: boolean;
};
