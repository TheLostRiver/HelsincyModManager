import { createContext, useCallback, useMemo, useState, type ReactNode } from "react";
import { buildNavigationState, resolveRoute } from "./routeCore";
import { appRoutes, enabledRouteIds } from "./routeRegistry";
import type { AppRoute, NavigationItemLike, NavigationStateItem } from "./routeTypes";

type AppRouteContextValue = {
  currentPath: string;
  currentRoute: AppRoute;
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
  const [currentPath, setCurrentPath] = useState(appRoutes[0].path);
  const currentRoute = resolveRoute(currentPath, appRoutes);

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
      getNavigationState: (items) =>
        buildNavigationState({
          currentPath: currentRoute.path,
          enabledRouteIds,
          items,
          routes: appRoutes,
        }),
      navigate,
    }),
    [currentRoute, navigate],
  );

  return <AppRouteContext.Provider value={value}>{children}</AppRouteContext.Provider>;
}
