import { useEffect, useMemo, useState, type AnimationEvent } from "react";
import {
  beginRouteTransition,
  completeRouteExit,
  createInitialRouteLayer,
  createRouteLayerKey,
  type RouteLayer,
} from "./routeTransition";
import { useAppRoute } from "./useAppRoute";

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

    if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }

    setRouteLayers((previousLayers) => completeRouteExit(previousLayers));
  }, [routeLayers]);

  function handleLayerAnimationEnd(event: AnimationEvent<HTMLDivElement>, phase: RouteLayer["phase"]) {
    if (event.currentTarget !== event.target) {
      return;
    }

    if (phase !== "exiting") {
      return;
    }

    setRouteLayers((previousLayers) => completeRouteExit(previousLayers));
  }

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
            data-tour-id={`page.${layer.route.id}`}
            onAnimationEnd={(event) => handleLayerAnimationEnd(event, layer.phase)}
          >
            <RouteElement />
          </div>
        );
      })}
    </div>
  );
}
