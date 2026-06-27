import { invoke } from "@tauri-apps/api/core";

export type UpdateModMetadataInput = {
  modId: string;
  displayName?: string;
  author?: string;
  version?: string;
  description?: string;
  nexusModId?: number;
};

export function updateModMetadata(input: UpdateModMetadataInput): Promise<void> {
  return invoke("update_mod_metadata", {
    modId: input.modId,
    displayName: input.displayName,
    author: input.author,
    version: input.version,
    description: input.description,
    nexusModId: input.nexusModId,
  });
}

export function deleteModMetadata(modId: string): Promise<void> {
  return invoke("delete_mod_metadata", { modId });
}
