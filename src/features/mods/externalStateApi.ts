// #286 external mod state scan transport wrappers.
//
// The summary DTO is shaped exactly like `ExternalInstallStateSummary` in
// `externalInstallStatusView.ts` (backend serializes snake_case state literals),
// so projection consumes the response without an extra mapping layer.

import { invoke } from "@tauri-apps/api/core";
import type { ExternalInstallStateSummary } from "./externalInstallStatusView";

export type ExternalStateScanTaskDto = {
  taskId: string;
  kind: string;
  status: string;
};

export type ExternalStateScanStartedDto = {
  task: ExternalStateScanTaskDto;
  modId: string;
};

export type ExternalModStateDto = {
  /** Last successful judgement; null when this mod was never scanned. */
  summary: ExternalInstallStateSummary | null;
  /** Facts may have drifted since the last scan (re-stat mismatch). */
  stale: boolean;
  /** Stable `external_state_scan_*` code of the last failed scan attempt. */
  lastError: string | null;
};

export type ExternalModStateRequest = {
  gameId: string;
  profileId: string;
  modId: string;
};

export async function startExternalModStateScan(
  input: ExternalModStateRequest,
): Promise<ExternalStateScanStartedDto> {
  return invoke<ExternalStateScanStartedDto>("start_external_mod_state_scan", {
    gameId: input.gameId,
    profileId: input.profileId,
    modId: input.modId,
  });
}

export async function getExternalModState(
  input: ExternalModStateRequest,
): Promise<ExternalModStateDto> {
  return invoke<ExternalModStateDto>("get_external_mod_state", {
    gameId: input.gameId,
    profileId: input.profileId,
    modId: input.modId,
  });
}
