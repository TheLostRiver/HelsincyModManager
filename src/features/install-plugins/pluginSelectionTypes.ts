export type PluginSelectionScope = { gameId: string; profileId: string; modId: string; revisionId?: string };
export type PluginCandidate = {
  fileId: string; relativePath: string; sizeBytes: number;
  check: "supported" | "invalid_format" | "unsupported_architecture" | "not_dynamic_library" | "policy_excluded";
  selected: boolean; selectable: boolean; managed: boolean; retainOnly: boolean; excludedByPackage: boolean;
};
export type PluginInventory = Required<PluginSelectionScope> & {
  inventoryId: string; confirmationRequired: boolean; files: PluginCandidate[];
};
export type PluginSelectionInput = Required<PluginSelectionScope> & { inventoryId: string; selectedFileIds: string[] };
