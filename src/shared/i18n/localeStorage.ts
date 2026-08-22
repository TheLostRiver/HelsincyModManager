import {
  defaultLocalePreference,
  isLocalePreference,
  type LocalePreference,
} from "./locales";

const storageKey = "helsincy.localePreference";

type PersistedLocaleSettings = {
  version: 1;
  preference: LocalePreference;
};

export function readPersistedLocalePreference(): LocalePreference {
  try {
    const rawValue = window.localStorage.getItem(storageKey);
    if (rawValue === null) {
      return defaultLocalePreference;
    }

    if (isLocalePreference(rawValue)) {
      return rawValue;
    }

    const parsedValue = JSON.parse(rawValue) as Partial<PersistedLocaleSettings>;
    return parsedValue.version === 1 && isLocalePreference(parsedValue.preference)
      ? parsedValue.preference
      : defaultLocalePreference;
  } catch {
    return defaultLocalePreference;
  }
}

export function writePersistedLocalePreference(preference: LocalePreference) {
  try {
    const value: PersistedLocaleSettings = { version: 1, preference };
    window.localStorage.setItem(storageKey, JSON.stringify(value));
  } catch {
    return;
  }
}
