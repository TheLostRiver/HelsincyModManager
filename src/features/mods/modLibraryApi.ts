import { invoke } from "@tauri-apps/api/core";
import type { GetModDetailInput, ModDetail, ModLibraryItem } from "./modLibraryTypes";

export function getModLibrary(): Promise<ModLibraryItem[]> {
  return invoke<ModLibraryItem[]>("get_mod_library");
}

export function getModDetail(input: GetModDetailInput): Promise<ModDetail | null> {
  return invoke<ModDetail | null>("get_mod_detail", {
    modId: input.modId,
  });
}
