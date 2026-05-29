import { invoke } from "@tauri-apps/api/core";
import type { AppHealth } from "../types/app";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}
