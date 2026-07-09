import { invoke } from "@tauri-apps/api/core";

export const WINDOW_CLOSE_REQUESTED_EVENT = "hmm://window-close-requested";

export function hideMainWindowToTray(): Promise<void> {
  return invoke<void>("hide_main_window_to_tray");
}

export function exitApplication(): Promise<void> {
  return invoke<void>("exit_app");
}
