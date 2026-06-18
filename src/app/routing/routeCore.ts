import type { NavigationItemLike, NavigationStateItem, RouteLike } from "./routeTypes";

const DEFAULT_ROUTE_INDEX = 0;

export function resolveRoute<TRoute extends RouteLike>(path: string, routes: readonly TRoute[]): TRoute {
  const exactMatch = routes.find((route) => route.path === path);
  const fallback = routes[DEFAULT_ROUTE_INDEX];

  if (!fallback) {
    throw new Error("At least one app route must be registered.");
  }

  return exactMatch ?? fallback;
}

type BuildNavigationStateOptions<TItem extends NavigationItemLike, TRoute extends RouteLike> = {
  currentPath: string;
  enabledRouteIds: ReadonlySet<string>;
  items: readonly TItem[];
  routes: readonly TRoute[];
};

export function buildNavigationState<TItem extends NavigationItemLike, TRoute extends RouteLike>({
  currentPath,
  enabledRouteIds,
  items,
  routes,
}: BuildNavigationStateOptions<TItem, TRoute>): NavigationStateItem<TItem>[] {
  const activeRoute = resolveRoute(currentPath, routes);
  const routeByPath = new Map(routes.map((route) => [route.path, route]));

  return items.map((item) => {
    const route = routeByPath.get(item.route);
    const isDisabled = route ? !enabledRouteIds.has(route.id) : true;

    return {
      ...item,
      isActive: route?.id === activeRoute.id,
      isDisabled,
    };
  });
}
