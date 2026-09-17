import { invoke } from "@tauri-apps/api/core";
import { trackModLibraryTaskStart } from "./modLibraryWriteTracking.ts";
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
  StartRecoveryActionTaskInput,
  StartUninstallTaskInput,
} from "./modInstallPlanTypes";

export function previewInstallPlanForImportedMod(
  input: PreviewImportedModInstallPlanInput,
): Promise<InstallPlanPreview> {
  return invoke<InstallPlanPreview>("preview_imported_mod_install_plan", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      ...(input.profileId === undefined ? {} : { profileId: input.profileId }),
      layerName: input.layerName,
      layerPriority: input.layerPriority,
    },
  });
}

export function startInstallTask(input: StartInstallTaskInput): Promise<TaskStartedDto> {
  return trackModLibraryTaskStart(input, () => invoke<TaskStartedDto>("start_install_task", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      profileId: input.profileId,
      layerName: input.layerName,
      layerPriority: input.layerPriority,
      ...(input.expectedRevisionId === undefined ? {} : { expectedRevisionId: input.expectedRevisionId }),
    },
  }));
}

export function startUninstallTask(input: StartUninstallTaskInput): Promise<TaskStartedDto> {
  return trackModLibraryTaskStart(input, () => invoke<TaskStartedDto>("start_uninstall_task", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      profileId: input.profileId,
    },
  }));
}

export function getInstallManifestStatus(
  input: GetInstallManifestStatusInput,
): Promise<InstallManifestStatusSummary[]> {
  return invoke<InstallManifestStatusSummary[]>("get_install_manifest_status", {
    request: {
      ...(input.gameId === undefined ? {} : { gameId: input.gameId }),
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

export function startRecoveryActionTask(input: StartRecoveryActionTaskInput): Promise<TaskStartedDto> {
  return trackModLibraryTaskStart(input, () => invoke<TaskStartedDto>("start_recovery_action_task", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modId: input.modId,
      actionKind: input.actionKind,
      ...(input.planToken ? { planToken: input.planToken } : {}),
    },
  }));
}
