export const colorSchemePreferences = ["light", "dark", "system"] as const;

export type ColorSchemePreference = (typeof colorSchemePreferences)[number];

export type EffectiveColorScheme = "light" | "dark";

export type PersistedColorSchemeSettings = {
  version: 1;
  preference: ColorSchemePreference;
};

export const defaultColorSchemePreference: ColorSchemePreference = "system";

export function isColorSchemePreference(value: unknown): value is ColorSchemePreference {
  return value === "light" || value === "dark" || value === "system";
}
