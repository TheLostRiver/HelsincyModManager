import { createContext, useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { buildNavigationState, resolveRoute } from "./routeCore";
import { appRoutes, enabledRouteIds } from "./routeRegistry";
import type { AppRoute, NavigationItemLike, NavigationStateItem } from "./routeTypes";
import {
  loadLastRouteId,
  loadStartPagePreference,
  saveLastRouteId,
  saveStartPagePreference,
  type StartPagePreference,
} from "./startPagePreference";

type AppRouteContextValue = {
  currentPath: string;
  currentRoute: AppRoute;
  startPagePreference: StartPagePreference;
  setStartPagePreference: (preference: StartPagePreference) => boolean;
  getNavigationState: <TItem extends NavigationItemLike>(
    items: readonly TItem[],
  ) => NavigationStateItem<TItem>[];
  navigate: (path: string) => void;
};

export const AppRouteContext = createContext<AppRouteContextValue | null>(null);

type AppRouteProviderProps = {
  children: ReactNode;
};

export function AppRouteProvider({ children }: AppRouteProviderProps) {
  const [startPagePreference, setSavedStartPagePreference] = useState(loadStartPagePreference);
  const [currentPath, setCurrentPath] = useState(() => {
    const routeId = startPagePreference === "last" ? loadLastRouteId() : startPagePreference;
    return appRoutes.find((route) => route.id === routeId && enabledRouteIds.has(route.id))?.path
      ?? appRoutes[0].path;
  });
  const currentRoute = resolveRoute(currentPath, appRoutes);

  useEffect(() => {
    saveLastRouteId(currentRoute.id);
  }, [currentRoute.id]);

  const setStartPagePreference = useCallback((preference: StartPagePreference) => {
    if (!saveStartPagePreference(preference)) return false;
    setSavedStartPagePreference(preference);
    return true;
  }, []);

  const navigate = useCallback((path: string) => {
    const targetRoute = resolveRoute(path, appRoutes);

    if (!enabledRouteIds.has(targetRoute.id)) {
      return;
    }

    setCurrentPath(targetRoute.path);
  }, []);

  const value = useMemo<AppRouteContextValue>(
    () => ({
      currentPath: currentRoute.path,
      currentRoute,
      startPagePreference,
      setStartPagePreference,
      getNavigationState: (items) =>
        buildNavigationState({
          currentPath: currentRoute.path,
          enabledRouteIds,
          items,
          routes: appRoutes,
        }),
      navigate,
    }),
    [currentRoute, navigate, setStartPagePreference, startPagePreference],
  );

  return <AppRouteContext.Provider value={value}>{children}</AppRouteContext.Provider>;
}
