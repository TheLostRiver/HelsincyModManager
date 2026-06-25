import { invoke } from "@tauri-apps/api/core";
import type { InstallPlanPreview, PreviewImportedModInstallPlanInput } from "./modInstallPlanTypes";

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
