import { invoke } from "@tauri-apps/api/core";
import type { BackgroundProtectionControlDto } from "./backgroundProtectionTypes";

let cachedControlStatus: BackgroundProtectionControlDto | null = null;
let pendingControlStatus: Promise<BackgroundProtectionControlDto> | null = null;

function retainControlStatus(control: BackgroundProtectionControlDto) {
  cachedControlStatus = control;
  return control;
}

export function peekBackgroundProtectionControlStatus(): BackgroundProtectionControlDto | null {
  return cachedControlStatus;
}

export function getBackgroundProtectionControlStatus(options?: {
  force?: boolean;
}): Promise<BackgroundProtectionControlDto> {
  if (!options?.force && cachedControlStatus) {
    return Promise.resolve(cachedControlStatus);
  }
  if (!options?.force && pendingControlStatus) {
    return pendingControlStatus;
  }

  const request = invoke<BackgroundProtectionControlDto>(
    "get_save_backup_background_control_status",
  ).then(retainControlStatus);
  pendingControlStatus = request;
  request.then(
    () => {
      if (pendingControlStatus === request) pendingControlStatus = null;
    },
    () => {
      if (pendingControlStatus === request) pendingControlStatus = null;
    },
  );
  return request;
}

export function enableBackgroundProtection(): Promise<BackgroundProtectionControlDto> {
  return invoke<BackgroundProtectionControlDto>(
    "enable_save_backup_background_protection",
  ).then(retainControlStatus);
}

export function disableBackgroundProtection(): Promise<BackgroundProtectionControlDto> {
  return invoke<BackgroundProtectionControlDto>(
    "disable_save_backup_background_protection",
  ).then(retainControlStatus);
}
