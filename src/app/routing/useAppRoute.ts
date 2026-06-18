import { useContext } from "react";
import { AppRouteContext } from "./AppRouteProvider";

export function useAppRoute() {
  const context = useContext(AppRouteContext);

  if (!context) {
    throw new Error("useAppRoute must be used inside AppRouteProvider.");
  }

  return context;
}
