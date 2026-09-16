import type { ModLibraryItem } from "./modLibraryTypes";

export const MOD_LIBRARY_SORTS = ["imported_at_desc", "imported_at_asc", "name_asc", "name_desc", "size_desc", "size_asc"] as const;
export type ModLibrarySort = typeof MOD_LIBRARY_SORTS[number];
export const MOD_LIBRARY_SORT_FIELDS = ["name", "imported_at", "size"] as const;
export type ModLibrarySortField = typeof MOD_LIBRARY_SORT_FIELDS[number];
export type ModLibrarySortDirection = "asc" | "desc";
export const DEFAULT_MOD_LIBRARY_SORT: ModLibrarySort = "imported_at_desc";
export const MOD_LIBRARY_SORT_STORAGE_KEY = "hmm.modLibrary.sort.v1";

export function modLibrarySortField(sort: ModLibrarySort): ModLibrarySortField {
  return sort.startsWith("name_") ? "name" : sort.startsWith("size_") ? "size" : "imported_at";
}

export function modLibrarySortDirection(sort: ModLibrarySort): ModLibrarySortDirection {
  return sort.endsWith("_asc") ? "asc" : "desc";
}

export function composeModLibrarySort(field: ModLibrarySortField, direction: ModLibrarySortDirection): ModLibrarySort {
  return `${field}_${direction}`;
}

export function isModLibrarySort(value: unknown): value is ModLibrarySort {
  return MOD_LIBRARY_SORTS.some((sort) => sort === value);
}

export function readModLibrarySort(storage: Pick<Storage, "getItem"> | null): ModLibrarySort {
  try {
    const value = storage?.getItem(MOD_LIBRARY_SORT_STORAGE_KEY);
    return isModLibrarySort(value) ? value : DEFAULT_MOD_LIBRARY_SORT;
  } catch { return DEFAULT_MOD_LIBRARY_SORT; }
}

export function writeModLibrarySort(storage: Pick<Storage, "setItem"> | null, sort: ModLibrarySort): ModLibrarySort {
  const value = isModLibrarySort(sort) ? sort : DEFAULT_MOD_LIBRARY_SORT;
  try { storage?.setItem(MOD_LIBRARY_SORT_STORAGE_KEY, value); } catch { /* Session choice still works. */ }
  return value;
}

export function normalizeModLibraryText(value: string): string {
  return Array.from(value.normalize("NFKC"), (character) => character.toLowerCase())
    .join("").split(/\p{White_Space}+/u).filter(Boolean).join(" ");
}

const encoder = new TextEncoder();

/** Browser preview only. Production consumes a page already ordered by the indexed query. */
export function modLibraryNameSortKey(value: string): Uint8Array {
  const bytes: number[] = [];
  for (const part of normalizeModLibraryText(value).match(/[0-9]+|[^0-9]+/gu) ?? []) {
    if (/^[0-9]/u.test(part)) {
      const digits = part.replace(/^0+/u, "") || "0";
      bytes.push(48, 0, 0, 0, 0,
        (digits.length >>> 24) & 255, (digits.length >>> 16) & 255,
        (digits.length >>> 8) & 255, digits.length & 255,
        ...encoder.encode(digits), 0);
    } else {
      bytes.push(...encoder.encode(part));
    }
  }
  return Uint8Array.from(bytes);
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < Math.min(left.length, right.length); index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return left.length - right.length;
}

export function knownModLibraryNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

export function sortBrowserModLibrary(items: readonly ModLibraryItem[], sort: ModLibrarySort): ModLibraryItem[] {
  return items.map((item) => ({ item, key: modLibraryNameSortKey(item.name), id: encoder.encode(item.id) }))
    .sort((left, right) => {
      const byName = compareBytes(left.key, right.key) || compareBytes(left.id, right.id);
      if (sort === "name_asc") return byName;
      if (sort === "name_desc") return -byName;
      const field = sort.startsWith("size_") ? "contentSizeBytes" : "importedAtUnixMillis";
      const a = left.item[field];
      const b = right.item[field];
      if (!knownModLibraryNumber(a)) return knownModLibraryNumber(b) ? 1 : byName;
      if (!knownModLibraryNumber(b)) return -1;
      return (sort.endsWith("_desc") ? b - a : a - b) || byName;
    }).map(({ item }) => item);
}

const numberFormats = new Map<string, Intl.NumberFormat>();
export function formatModContentSize(bytes: unknown, locale: string): string | null {
  if (!knownModLibraryNumber(bytes)) return null;
  let formatter = numberFormats.get(locale);
  if (!formatter) {
    formatter = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
    numberFormats.set(locale, formatter);
  }
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const unit = bytes === 0 ? 0 : Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  return `${formatter.format(bytes / 1024 ** unit)} ${units[unit]}`;
}
