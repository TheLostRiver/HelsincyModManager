import type { ModDetail, ModLibraryItem } from "./modLibraryTypes";
import type { ModReplacementSummary } from "../replacements/replacementTypes";

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function modHoverModel(detail: ModDetail, item: ModLibraryItem) {
  const notes = text(detail.description)?.replace(/\s+/gu, " ") ?? null;
  const characters = notes ? Array.from(notes) : [];
  const categories = item.categoryLabels.map((label) => text(label.name)).filter((name): name is string => name !== null);
  const inferred = text(detail.metadata.category);
  return {
    name: text(detail.name) ?? item.name,
    author: text(detail.metadata.author),
    version: text(detail.metadata.version),
    notes: characters.length > 160 ? `${characters.slice(0, 160).join("")}...` : notes,
    categories: [...new Set(categories.length ? categories : inferred ? [inferred] : [])],
    tags: [...new Set(detail.metadata.tags.map(text).filter((tag): tag is string => tag !== null))],
    nexusId: typeof detail.nexusModId === "number" && Number.isSafeInteger(detail.nexusModId) && detail.nexusModId > 0
      ? String(detail.nexusModId) : null,
  };
}

export function coherentHoverReplacement(
  detail: ModDetail | null,
  summary: ModReplacementSummary | null,
  gameId: string,
): ModReplacementSummary | null {
  return detail && summary && summary.modId === detail.id && summary.gameId === gameId && summary.packageId === detail.packageId
    ? summary : null;
}
