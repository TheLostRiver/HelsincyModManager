import { useAppRoute } from "./useAppRoute";

export function RouterOutlet() {
  const { currentRoute } = useAppRoute();
  const RouteElement = currentRoute.element;

  return <RouteElement />;
}
