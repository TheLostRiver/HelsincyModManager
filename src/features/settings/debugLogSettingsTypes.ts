// 直连 locales 而不是 shared/i18n barrel，保证可被 node --test 直接 import（无 JSX 链）。
import { resolveCopy, type Locale } from "../../shared/i18n/locales";
import { debugLogSettingsCopy } from "./debugLogSettingsCopy";

export type DebugLogSettingsDto = {
  enabled: boolean;
};

export type DebugLogSettingsState =
  | { status: "loading" }
  | { status: "ready"; settings: DebugLogSettingsDto; errorCode: string | null }
  | { status: "error"; errorCode: string };

export function getDebugLogErrorCode(error: unknown): string {
  if (!error || typeof error !== "object" || !("code" in error)) {
    return "unknown";
  }

  const code = (error as { code?: unknown }).code;
  return typeof code === "string" && code.trim() ? code : "unknown";
}

export function getDebugLogErrorMessage(code: string, locale: Locale): string {
  const errors = resolveCopy(debugLogSettingsCopy, locale).errors;
  switch (code) {
    case "configuration_database_unavailable":
    case "app_settings_unavailable":
      return errors.unavailableRetry;
    case "app_settings_save_failed":
      return errors.saveFailed;
    default:
      return errors.unavailableRecheck;
  }
}
