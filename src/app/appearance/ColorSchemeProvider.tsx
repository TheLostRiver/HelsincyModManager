import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  readPersistedColorSchemePreference,
  writePersistedColorSchemePreference,
} from "./colorSchemeStorage";
import type { ColorSchemePreference, EffectiveColorScheme } from "./colorSchemeTypes";

type ColorSchemeContextValue = {
  preference: ColorSchemePreference;
  effective: EffectiveColorScheme;
  setPreference: (preference: ColorSchemePreference) => void;
};

export const ColorSchemeContext = createContext<ColorSchemeContextValue | null>(null);

type ColorSchemeProviderProps = {
  children: ReactNode;
};

export function ColorSchemeProvider({ children }: ColorSchemeProviderProps) {
  const [preference, setPreferenceState] = useState<ColorSchemePreference>(
    readPersistedColorSchemePreference,
  );
  const [systemScheme, setSystemScheme] = useState<EffectiveColorScheme>(readSystemColorScheme);

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = (event: MediaQueryListEvent) => {
      setSystemScheme(event.matches ? "dark" : "light");
    };

    setSystemScheme(query.matches ? "dark" : "light");
    query.addEventListener("change", handleChange);
    return () => query.removeEventListener("change", handleChange);
  }, []);

  const effective = preference === "system" ? systemScheme : preference;

  useEffect(() => {
    document.documentElement.dataset.colorScheme = effective;
  }, [effective]);

  const setPreference = useCallback((nextPreference: ColorSchemePreference) => {
    setPreferenceState(nextPreference);
    writePersistedColorSchemePreference(nextPreference);
  }, []);

  const value = useMemo(
    () => ({ effective, preference, setPreference }),
    [effective, preference, setPreference],
  );

  return <ColorSchemeContext.Provider value={value}>{children}</ColorSchemeContext.Provider>;
}

function readSystemColorScheme(): EffectiveColorScheme {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
