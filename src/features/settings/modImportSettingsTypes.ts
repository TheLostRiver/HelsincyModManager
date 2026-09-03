// 直连 locales 而不是 shared/i18n barrel，且带 .ts 显式扩展：本模块会被 node --test 直接 import。
import { resolveCopy, type Locale } from "../../shared/i18n/locales.ts";
import { modImportSettingsCopy } from "./modImportSettingsCopy.ts";

export type ModImportSettingsDto = {
  deleteArchiveAfterImport: boolean;
};

export type ModImportSettingsState =
  | { status: "loading" }
  | { status: "ready"; settings: ModImportSettingsDto; saveFailed: boolean }
  | { status: "error" };

export function isModImportSettingsDto(value: unknown): value is ModImportSettingsDto {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { deleteArchiveAfterImport?: unknown }).deleteArchiveAfterImport === "boolean"
  );
}

/**
 * The backend only ever answers `app_settings_unavailable` here, so the message is chosen by
 * what the user was doing (reading vs. saving) rather than by pretending to map codes.
 */
export function getModImportSettingsErrorMessage(context: "load" | "save", locale: Locale): string {
  const errors = resolveCopy(modImportSettingsCopy, locale).errors;
  return context === "save" ? errors.saveFailed : errors.unavailableRetry;
}
