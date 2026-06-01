import {
  defaultColorSchemePreference,
  isColorSchemePreference,
  type ColorSchemePreference,
  type PersistedColorSchemeSettings,
} from "./colorSchemeTypes";

const storageKey = "helsincy.colorSchemePreference";

export function readPersistedColorSchemePreference(): ColorSchemePreference {
  try {
    const rawValue = window.localStorage.getItem(storageKey);
    if (rawValue === null) {
      return defaultColorSchemePreference;
    }

    if (isColorSchemePreference(rawValue)) {
      return rawValue;
    }

    const parsedValue = JSON.parse(rawValue) as Partial<PersistedColorSchemeSettings>;
    return parsedValue.version === 1 && isColorSchemePreference(parsedValue.preference)
      ? parsedValue.preference
      : defaultColorSchemePreference;
  } catch {
    return defaultColorSchemePreference;
  }
}

export function writePersistedColorSchemePreference(preference: ColorSchemePreference) {
  try {
    const value: PersistedColorSchemeSettings = { version: 1, preference };
    window.localStorage.setItem(storageKey, JSON.stringify(value));
  } catch {
    return;
  }
}
