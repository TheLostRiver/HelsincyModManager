import { invoke } from "@tauri-apps/api/core";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import type { AnalyzeImportedModReplacementInput, RetargetInstallTaskStarted } from "./replacementTypes";
import type { EquipmentRetargetConfiguration, EquipmentRetargetInstallPreview, EquipmentRetargetSelection } from "./equipmentRetargetTypes";

export function getEquipmentRetargetConfiguration(input: AnalyzeImportedModReplacementInput): Promise<EquipmentRetargetConfiguration> {
  return invoke("get_equipment_retarget_configuration", { request: { gameId: input.gameId, profileId: input.profileId, modId: input.modId } });
}

function selectionRequest(input: EquipmentRetargetSelection) {
  return {
    gameId: input.gameId, profileId: input.profileId, modId: input.modId,
    slots: input.slots.map((slot) => slot.action === "keep"
      ? { action: "keep", sourceId: slot.sourceId }
      : { action: "retarget", sourceId: slot.sourceId, targetId: slot.targetId }),
    layerName: input.layerName, layerPriority: input.layerPriority,
  };
}

export function previewEquipmentRetargetInstall(input: EquipmentRetargetSelection): Promise<EquipmentRetargetInstallPreview> {
  return invoke("preview_equipment_retarget_install", { request: selectionRequest(input) });
}

export function previewEquipmentRetargetReinstall(input: EquipmentRetargetSelection): Promise<ReinstallPlanPreview> {
  return invoke("preview_equipment_retarget_reinstall", { request: selectionRequest(input) });
}

export function startEquipmentRetargetInstall(input: EquipmentRetargetSelection): Promise<RetargetInstallTaskStarted> {
  return invoke("start_equipment_retarget_install_task", { request: selectionRequest(input) });
}

export function startEquipmentRetargetReinstall(input: EquipmentRetargetSelection, planToken: string): Promise<RetargetInstallTaskStarted> {
  return invoke("start_equipment_retarget_reinstall_task", { request: { selection: selectionRequest(input), planToken } });
}
