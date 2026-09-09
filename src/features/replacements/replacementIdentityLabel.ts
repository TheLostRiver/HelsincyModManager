import { resolveCopy, type Locale } from "../../shared/i18n/locales.ts";
import { resolveReplacementTargetNames } from "./replacementTargetNames.ts";
import { replacementSummaryCopy } from "./replacementSummaryCopy.ts";

export function replacementIdentityLabel(item: { internalId: string; displayNames?: Record<string, string> }, locale: Locale): string {
  const name = resolveReplacementTargetNames(item.displayNames ?? {}, locale).displayName;
  return `${name.trim() || resolveCopy(replacementSummaryCopy, locale).unknownName} (${item.internalId})`;
}

export function replacementKindLabel(kind: string, locale: Locale): string {
  const kinds: Record<string, string> = resolveCopy(replacementSummaryCopy, locale).kinds;
  return kinds[kind] ?? kind;
}
