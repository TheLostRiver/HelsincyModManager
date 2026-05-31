import { useContext } from "react";
import { SidebarModeContext } from "./SidebarModeProvider";

export function useSidebarMode() {
  const value = useContext(SidebarModeContext);

  if (value === null) {
    throw new Error("useSidebarMode must be used within SidebarModeProvider");
  }

  return value;
}
