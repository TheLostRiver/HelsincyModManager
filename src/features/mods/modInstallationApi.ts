import { invoke } from "@tauri-apps/api/core";

export type ModInstallationContext = {
  gameId: string;
  installationId: string;
  /** Backend-owned namespace; legacy install DTOs carry it in their profileId field. */
  scopeId: string;
};

export function getModInstallationContext(gameId: string): Promise<ModInstallationContext> {
  return invoke("get_mod_installation_context", { gameId });
}
