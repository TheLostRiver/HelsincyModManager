export const CLOSE_BEHAVIOR_STORAGE_KEY = "hmm.windowCloseBehavior";

export type WindowClosePreference = "ask" | "tray" | "exit";
export type WindowCloseAction = "show_dialog" | "hide_to_tray" | "exit_app";

type PreferenceStorage = Pick<Storage, "getItem" | "setItem"> | undefined;

const VALID_PREFERENCES = new Set<WindowClosePreference>(["ask", "tray", "exit"]);

function defaultStorage(): PreferenceStorage {
  return typeof window === "undefined" ? undefined : window.localStorage;
}

export function parseWindowClosePreference(value: unknown): WindowClosePreference {
  return typeof value === "string" && VALID_PREFERENCES.has(value as WindowClosePreference)
    ? (value as WindowClosePreference)
    : "ask";
}

export function loadWindowClosePreference(storage: PreferenceStorage = defaultStorage()): WindowClosePreference {
  if (!storage) return "ask";
  try {
    return parseWindowClosePreference(JSON.parse(storage.getItem(CLOSE_BEHAVIOR_STORAGE_KEY) ?? "null"));
  } catch {
    return "ask";
  }
}

export function saveWindowClosePreference(
  storage: PreferenceStorage = defaultStorage(),
  preference: WindowClosePreference,
): boolean {
  if (!storage) return false;
  try {
    storage.setItem(CLOSE_BEHAVIOR_STORAGE_KEY, JSON.stringify(parseWindowClosePreference(preference)));
    return true;
  } catch {
    return false;
  }
}

export function resolveWindowCloseAction(preference: WindowClosePreference): WindowCloseAction {
  if (preference === "tray") return "hide_to_tray";
  if (preference === "exit") return "exit_app";
  return "show_dialog";
}
