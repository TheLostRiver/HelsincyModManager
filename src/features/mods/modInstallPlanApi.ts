import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "./modImportTypes";
import type {
  InstallPlanPreview,
  PreviewImportedModInstallPlanInput,
  StartInstallTaskInput,
} from "./modInstallPlanTypes";

export function previewInstallPlanForImportedMod(
  input: PreviewImportedModInstallPlanInput,
): Promise<InstallPlanPreview> {
  return invoke<InstallPlanPreview>("preview_imported_mod_install_plan", {
    gameId: input.gameId,
    modId: input.modId,
    layerName: input.layerName,
    layerPriority: input.layerPriority,
  });
}

export function startInstallTask(input: StartInstallTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_install_task", {
    gameId: input.gameId,
    modId: input.modId,
    profileId: input.profileId,
    layerName: input.layerName,
    layerPriority: input.layerPriority,
  });
}
