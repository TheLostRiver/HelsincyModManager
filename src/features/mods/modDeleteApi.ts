import { invoke } from "@tauri-apps/api/core";

export type ModDeletionPreview = {
  modId: string;
  displayName: string;
  revisionCount: number;
  categoryLabels: string[];
  affectedProfiles: string[];
};

export type ModDeletionResult = {
  modId: string;
  removedRevisionCount: number;
  removedPackageIds: string[];
};

export function previewModDeletion(modId: string): Promise<ModDeletionPreview> {
  return invoke("preview_mod_deletion", { modId });
}

export function deleteModFromLibrary(modId: string): Promise<ModDeletionResult> {
  return invoke("delete_mod_from_library", { modId });
}
