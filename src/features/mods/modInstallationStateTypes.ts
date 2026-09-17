import type { InstallManifestStatusSummary } from "./modInstallPlanTypes";

export const MOD_INSTALLATION_STATE_EVENT = "hmm://mod-installation-state";
export const MAX_MOD_INSTALLATION_STATE_IDS = 2048;

export type ModInstallationStateRequest = { gameId: string; profileId: string; modIds: string[] };
export type ModInstallationStateUpdate = ModInstallationStateRequest & {
  epoch: string;
  revision: number;
  reset: boolean;
  available: boolean;
  summaries: InstallManifestStatusSummary[];
};
export type ModInstallationStateEvent = ModInstallationStateUpdate & { taskId: string };
