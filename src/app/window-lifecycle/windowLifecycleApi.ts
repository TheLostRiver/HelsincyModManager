import { invoke } from "@tauri-apps/api/core";

export const WINDOW_CLOSE_REQUESTED_EVENT = "hmm://window-close-requested";

export type AppExitGuardReason =
  | "background_starting"
  | "background_not_enabled"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform"
  | "status_unavailable";

export type AppExitGuardDto =
  | { decision: "safe"; reason: null }
  | { decision: "confirmation_required"; reason: AppExitGuardReason };

export function hideMainWindowToTray(): Promise<void> {
  return invoke<void>("hide_main_window_to_tray");
}

export function getAppExitGuard(): Promise<AppExitGuardDto> {
  return invoke<AppExitGuardDto>("get_app_exit_guard");
}

export function exitApplication(overrideUnprotected = false): Promise<void> {
  return invoke<void>("exit_app", {
    request: { overrideUnprotected },
  });
}
