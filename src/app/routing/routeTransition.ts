import type { AppRoute } from "./routeTypes";

export type RouteLayerPhase = "active" | "entering" | "exiting";

export type RouteLayer = {
  key: string;
  route: AppRoute;
  phase: RouteLayerPhase;
};

export function createRouteLayerKey(route: AppRoute): string {
  return `${route.id}:${route.path}`;
}

function findLastVisibleLayer(layers: readonly RouteLayer[]): RouteLayer | undefined {
  for (let index = layers.length - 1; index >= 0; index -= 1) {
    const layer = layers[index];

    if (layer.phase !== "exiting") {
      return layer;
    }
  }

  return undefined;
}

export function createInitialRouteLayer(route: AppRoute): RouteLayer {
  return {
    key: createRouteLayerKey(route),
    route,
    phase: "active",
  };
}

export function beginRouteTransition(currentLayers: readonly RouteLayer[], targetRoute: AppRoute): RouteLayer[] {
  const targetKey = createRouteLayerKey(targetRoute);
  const existingTargetLayer = currentLayers.find((layer) => layer.key === targetKey);
  const visibleLayer = currentLayers.find((layer) => layer.phase !== "exiting") ?? currentLayers.at(-1);

  if (!visibleLayer || existingTargetLayer) {
    return [createInitialRouteLayer(targetRoute)];
  }

  return [
    {
      ...visibleLayer,
      phase: "exiting",
    },
    {
      key: targetKey,
      route: targetRoute,
      phase: "entering",
    },
  ];
}

export function completeRouteExit(currentLayers: readonly RouteLayer[]): RouteLayer[] {
  const visibleLayer = findLastVisibleLayer(currentLayers);

  if (!visibleLayer) {
    return [];
  }

  return [
    {
      ...visibleLayer,
      phase: "active",
    },
  ];
}
