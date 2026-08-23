import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  readPersistedLocalePreference,
  writePersistedLocalePreference,
} from "./localeStorage";
import {
  localeMeta,
  resolveSystemLocale,
  type Locale,
  type LocalePreference,
} from "./locales";

type I18nContextValue = {
  /** 生效语言：preference 为 system 时等于 systemLocale。 */
  locale: Locale;
  preference: LocalePreference;
  /** 系统语言解析结果；设置页「跟随系统（…）」括注展示用。 */
  systemLocale: Locale;
  setPreference: (preference: LocalePreference) => void;
};

export const I18nContext = createContext<I18nContextValue | null>(null);

type I18nProviderProps = {
  children: ReactNode;
};

export function I18nProvider({ children }: I18nProviderProps) {
  const [preference, setPreferenceState] = useState<LocalePreference>(
    readPersistedLocalePreference,
  );
  const [systemLocale, setSystemLocale] = useState<Locale>(readSystemLocale);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const handleLanguageChange = () => {
      setSystemLocale(readSystemLocale());
    };

    window.addEventListener("languagechange", handleLanguageChange);
    return () => window.removeEventListener("languagechange", handleLanguageChange);
  }, []);

  const locale = preference === "system" ? systemLocale : preference;

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    // 读屏语言与日期/数字格式以生效语言为准。
    document.documentElement.lang = localeMeta[locale].bcp47;
  }, [locale]);

  const setPreference = useCallback((nextPreference: LocalePreference) => {
    setPreferenceState(nextPreference);
    writePersistedLocalePreference(nextPreference);
  }, []);

  const value = useMemo(
    () => ({ locale, preference, systemLocale, setPreference }),
    [locale, preference, systemLocale, setPreference],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

function readSystemLocale(): Locale {
  if (typeof navigator === "undefined") {
    return "en";
  }

  const tags =
    navigator.languages && navigator.languages.length > 0
      ? navigator.languages
      : [navigator.language];
  return resolveSystemLocale(tags.filter(Boolean));
}
