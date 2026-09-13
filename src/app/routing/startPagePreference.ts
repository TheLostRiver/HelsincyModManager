export type StartPagePreference = "dashboard" | "mods" | "last";

export const START_PAGE_STORAGE_KEY = "hmm.startPage";
export const LAST_ROUTE_STORAGE_KEY = "hmm.lastRouteId";

type PreferenceStorage = Pick<Storage, "getItem" | "setItem"> | undefined;

function browserStorage(): PreferenceStorage {
  try {
    return typeof window === "undefined" ? undefined : window.localStorage;
  } catch {
    return undefined;
  }
}

function readValue(key: string, storage: PreferenceStorage) {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function saveValue(key: string, value: string, storage: PreferenceStorage) {
  try {
    if (!storage) return false;
    storage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

export function loadStartPagePreference(storage = browserStorage()): StartPagePreference {
  const value = readValue(START_PAGE_STORAGE_KEY, storage);
  return value === "mods" || value === "last" ? value : "dashboard";
}

export function saveStartPagePreference(value: StartPagePreference, storage = browserStorage()) {
  if (value !== "dashboard" && value !== "mods" && value !== "last") return false;
  return saveValue(START_PAGE_STORAGE_KEY, value, storage);
}

export function loadLastRouteId(storage = browserStorage()) {
  return readValue(LAST_ROUTE_STORAGE_KEY, storage);
}

export function saveLastRouteId(routeId: string, storage = browserStorage()) {
  return saveValue(LAST_ROUTE_STORAGE_KEY, routeId, storage);
}
