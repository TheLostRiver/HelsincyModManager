import { invoke } from "@tauri-apps/api/core";
import type { AppHealth } from "../types/app";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}

export {
  startImportModTask,
} from "../../features/mods/modImportApi";

export {
  autoDetectGameDirectory,
  getGameSetupStatus,
  saveGameDirectory,
  scanGameCandidates,
  validateGameDirectory,
} from "../../features/game-setup/gameSetupApi";

export { getGamePrerequisiteStatus } from "../../features/game-setup/gamePrerequisiteApi";
