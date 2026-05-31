export type SidebarMode = "classic" | "floating";

export type PersistedSidebarModeSettings = {
  version: 1;
  sidebarMode: SidebarMode;
};

export const defaultSidebarMode: SidebarMode = "classic";
