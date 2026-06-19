import { useEffect, useMemo, useState } from "react";
import {
  beginRouteTransition,
  completeRouteExit,
  createInitialRouteLayer,
  createRouteLayerKey,
  type RouteLayer,
} from "./routeTransition";
import { useAppRoute } from "./useAppRoute";

const routeExitDurationMs = 240;

function getNewestVisibleLayer(layers: readonly RouteLayer[]): RouteLayer | undefined {
  for (let index = layers.length - 1; index >= 0; index -= 1) {
    const layer = layers[index];

    if (layer.phase !== "exiting") {
      return layer;
    }
  }

  return undefined;
}

export function RouterOutlet() {
  const { currentRoute } = useAppRoute();
  const currentRouteKey = createRouteLayerKey(currentRoute);
  const [routeLayers, setRouteLayers] = useState<RouteLayer[]>(() => [createInitialRouteLayer(currentRoute)]);

  const visibleRouteKey = useMemo(() => getNewestVisibleLayer(routeLayers)?.key, [routeLayers]);

  useEffect(() => {
    if (visibleRouteKey === currentRouteKey) {
      return;
    }

    setRouteLayers((previousLayers) => beginRouteTransition(previousLayers, currentRoute));
  }, [currentRoute, currentRouteKey, visibleRouteKey]);

  useEffect(() => {
    if (!routeLayers.some((layer) => layer.phase === "exiting")) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setRouteLayers((previousLayers) => completeRouteExit(previousLayers));
    }, routeExitDurationMs);

    return () => window.clearTimeout(timeoutId);
  }, [routeLayers]);

  return (
    <div className="route-transition" aria-live="polite">
      {routeLayers.map((layer) => {
        const RouteElement = layer.route.element;
        const isHiddenFromA11y = layer.phase === "exiting";

        return (
          <div
            key={layer.key}
            className={`route-transition__layer is-${layer.phase}`}
            aria-hidden={isHiddenFromA11y || undefined}
            inert={isHiddenFromA11y ? true : undefined}
            data-route-id={layer.route.id}
          >
            <RouteElement />
          </div>
        );
      })}
    </div>
  );
}
