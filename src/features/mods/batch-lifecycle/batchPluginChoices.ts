import { getModPluginSelection } from "../../install-plugins/pluginSelectionApi.ts";
import type { PluginInventory } from "../../install-plugins/pluginSelectionTypes";
import type { BatchModLifecycleRequestDto } from "./batchModLifecycleTypes";

export async function loadBatchPluginChoices(request: BatchModLifecycleRequestDto, isCurrent: () => boolean): Promise<PluginInventory[]> {
  const choices: PluginInventory[] = [];
  // Avoid starting a scan for every row at once in large batches.
  for (const item of request.items) {
    if (!isCurrent()) return [];
    if (item.operation === "uninstall") continue;
    const inventory = await getModPluginSelection({ gameId: request.gameId, profileId: request.profileId,
      modId: item.modId, revisionId: item.operation === "install" ? item.revisionId : item.candidateRevisionId });
    if (inventory) choices.push(inventory);
  }
  return choices;
}
