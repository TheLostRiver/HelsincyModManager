import { invoke } from "@tauri-apps/api/core";
import type { ModInstallationStateRequest, ModInstallationStateUpdate } from "./modInstallationStateTypes";

export function getModInstallationStates(request: ModInstallationStateRequest): Promise<ModInstallationStateUpdate> {
  return invoke<ModInstallationStateUpdate>("get_mod_installation_states", { request });
}
