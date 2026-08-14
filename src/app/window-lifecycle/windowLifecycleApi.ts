import { invoke } from "@tauri-apps/api/core";

export const WINDOW_CLOSE_REQUESTED_EVENT = "hmm://window-close-requested";

export type SaveBackupExitGuardReason =
  | "background_starting"
  | "background_not_enabled"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform"
  | "status_unavailable";

export type AppExitBlockReason =
  | "save_restore_in_progress"
  | "save_restore_status_unavailable";

export type AppExitGuardDto =
  | { decision: "safe"; reason: null; exitAuthorization: null }
  | {
      decision: "confirmation_required";
      reason: SaveBackupExitGuardReason;
      exitAuthorization: string;
    }
  | { decision: "blocked"; reason: AppExitBlockReason; exitAuthorization: null };

export type ExitAppResultDto =
  | { outcome: "exiting"; reason: null; exitAuthorization: null }
  | {
      outcome: "confirmation_required";
      reason: SaveBackupExitGuardReason;
      exitAuthorization: string;
    }
  | { outcome: "blocked"; reason: AppExitBlockReason; exitAuthorization: null };

export function hideMainWindowToTray(): Promise<void> {
  return invoke<void>("hide_main_window_to_tray");
}

export function getAppExitGuard(): Promise<AppExitGuardDto> {
  return invoke<AppExitGuardDto>("get_app_exit_guard");
}

export function exitApplication(
  overrideUnprotected = false,
  exitAuthorization?: string,
): Promise<ExitAppResultDto> {
  return invoke<ExitAppResultDto>("exit_app", {
    request: { overrideUnprotected, exitAuthorization },
  });
}
