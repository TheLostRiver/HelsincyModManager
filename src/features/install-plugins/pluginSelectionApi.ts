import { invoke } from "@tauri-apps/api/core";
import type { PluginInventory, PluginSelectionInput, PluginSelectionScope } from "./pluginSelectionTypes";

export function getModPluginSelection(scope: PluginSelectionScope): Promise<PluginInventory | null> {
  return invoke("get_mod_plugin_selection", { request: { gameId: scope.gameId, profileId: scope.profileId, modId: scope.modId, revisionId: scope.revisionId ?? null } });
}

export function setModPluginSelection(input: PluginSelectionInput): Promise<PluginInventory> {
  return invoke("set_mod_plugin_selection", { request: { gameId: input.gameId, profileId: input.profileId, modId: input.modId,
    revisionId: input.revisionId, inventoryId: input.inventoryId, selectedFileIds: input.selectedFileIds } });
}

export function selectionInput(inventory: PluginInventory): PluginSelectionInput {
  return { gameId: inventory.gameId, profileId: inventory.profileId, modId: inventory.modId, revisionId: inventory.revisionId,
    inventoryId: inventory.inventoryId, selectedFileIds: inventory.files.filter((file) => file.selected).map((file) => file.fileId) };
}
