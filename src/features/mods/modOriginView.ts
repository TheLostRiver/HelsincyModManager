import { localeMeta, resolveCopy, type Locale } from "../../shared/i18n/locales.ts";
import { externalImportCopy } from "./external-import/externalImportCopy.ts";
import { modDetailDialogCopy } from "./modDetailDialogCopy.ts";
import type { ModOrigin } from "./modLibraryTypes";

export function modOriginLabel(origin: ModOrigin | null | undefined, locale: Locale): string {
  const copy = resolveCopy(modDetailDialogCopy, locale);
  if (origin?.kind === "imported") return copy.originImported;
  if (origin?.kind === "migrated_v1") return copy.originMigrated;
  if (origin?.kind !== "external_import") return copy.originUnspecified;
  const adapters: Record<string, string> = resolveCopy(externalImportCopy, locale).history.adapters;
  const source = (origin.adapterId ? adapters[origin.adapterId] : undefined) ?? copy.originUnknownSource;
  const date = origin.importedAtUnixMillis === null ? null : new Date(origin.importedAtUnixMillis);
  const importedAt = date && Number.isFinite(date.getTime())
    ? date.toLocaleDateString(localeMeta[locale].bcp47) : "-";
  return copy.originExternalImport(source, importedAt);
}
