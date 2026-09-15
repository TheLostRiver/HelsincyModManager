import { invoke } from "@tauri-apps/api/core";

export function openModFolder(modId: string): Promise<void> {
  return invoke("open_mod_folder", { modId });
}

export function openModNexusPage(modId: string): Promise<void> {
  return invoke("open_mod_nexus_page", { modId });
}
