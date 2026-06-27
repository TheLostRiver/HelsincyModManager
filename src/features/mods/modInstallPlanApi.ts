import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "./modImportTypes";
import type {
  GetInstallManifestStatusInput,
  InstallRecoveryActionPreview,
  InstallManifestStatusSummary,
  InstallPlanPreview,
  InstallRecoverySummary,
  PreviewImportedModInstallPlanInput,
  PreviewRecoveryActionInput,
  ScanInstallRecoveryInput,
  StartInstallTaskInput,
  StartUninstallTaskInput,
} from "./modInstallPlanTypes";

export function previewInstallPlanForImportedMod(
  input: PreviewImportedModInstallPlanInput,
): Promise<InstallPlanPreview> {
  return invoke<InstallPlanPreview>("preview_imported_mod_install_plan", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      layerName: input.layerName,
      layerPriority: input.layerPriority,
    },
  });
}

export function startInstallTask(input: StartInstallTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_install_task", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      profileId: input.profileId,
      layerName: input.layerName,
      layerPriority: input.layerPriority,
    },
  });
}

export function startUninstallTask(input: StartUninstallTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_uninstall_task", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      profileId: input.profileId,
    },
  });
}

export function getInstallManifestStatus(
  input: GetInstallManifestStatusInput,
): Promise<InstallManifestStatusSummary[]> {
  return invoke<InstallManifestStatusSummary[]>("get_install_manifest_status", {
    request: {
      profileId: input.profileId,
      modIds: input.modIds,
    },
  });
}

export function scanInstallRecovery(input: ScanInstallRecoveryInput): Promise<InstallRecoverySummary[]> {
  return invoke<InstallRecoverySummary[]>("scan_install_recovery", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modIds: input.modIds,
    },
  });
}

export function previewRecoveryAction(input: PreviewRecoveryActionInput): Promise<InstallRecoveryActionPreview> {
  return invoke<InstallRecoveryActionPreview>("preview_recovery_action", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modId: input.modId,
      actionKind: input.actionKind,
    },
  });
}
