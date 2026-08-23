// 全仓唯一的 locale 定义点（I18N_DESIGN.md 关键决策 2）：新增语言只改本文件 + 补各
// feature 字典；任何组件不得自带语言判断或硬编码语言自称名。

export const coreLocales = ["zh_cn", "en", "ja"] as const;

export type Locale = (typeof coreLocales)[number];

export type LocalePreference = Locale | "system";

export type LocaleMeta = {
  locale: Locale;
  /** 语言自称名：切换 UI 永远显示这一列，不随界面语言翻译。 */
  nativeName: string;
  /** BCP 47 tag，用于 `<html lang>` 与日期/数字格式化。 */
  bcp47: string;
  /** 取词兜底链（不含自身）；链终点统一为 en。 */
  fallback: readonly Locale[];
};

export const localeMeta: Record<Locale, LocaleMeta> = {
  zh_cn: { locale: "zh_cn", nativeName: "简体中文", bcp47: "zh-CN", fallback: ["en"] },
  en: { locale: "en", nativeName: "English", bcp47: "en", fallback: [] },
  ja: { locale: "ja", nativeName: "日本語", bcp47: "ja", fallback: ["en"] },
};

/** 默认中文以保持现状；「跟随系统」是一等选项而非默认值（I18N_DESIGN.md）。 */
export const defaultLocalePreference: LocalePreference = "zh_cn";

export function isLocale(value: unknown): value is Locale {
  return (coreLocales as readonly unknown[]).includes(value);
}

export function isLocalePreference(value: unknown): value is LocalePreference {
  return value === "system" || isLocale(value);
}

/**
 * 系统语言标签（`navigator.languages`）→ 受支持 locale；映射不到统一落 en。
 * zh-TW/zh-HK 目前没有独立字典，按 fallback 精神先落简中；未来繁中入列后改这里的映射。
 */
export function resolveSystemLocale(languageTags: readonly string[]): Locale {
  for (const tag of languageTags) {
    const normalized = tag.toLowerCase();
    if (normalized.startsWith("zh")) {
      return "zh_cn";
    }
    if (normalized.startsWith("ja")) {
      return "ja";
    }
    if (normalized.startsWith("en")) {
      return "en";
    }
  }
  return "en";
}

/**
 * 每 feature 字典的标准形态：核心语言必须全量（`satisfies LocaleDictionary<T>` 编译期锁死，
 * 缺任一语言的 key 直接编译失败）。
 */
export type LocaleDictionary<TCopy> = Record<Locale, TCopy>;

/**
 * 取词入口：所有调用方经此函数拿字典，不直接下标。当前核心语言全量时等价于直接下标；
 * 未来扩展语言允许 Partial 字典时，这里沿 fallback 链兜底，调用方不需要任何改动。
 */
export function resolveCopy<TCopy>(
  dictionary: LocaleDictionary<TCopy> & Partial<Record<string, TCopy>>,
  locale: Locale,
): TCopy {
  const direct = dictionary[locale];
  if (direct !== undefined) {
    return direct;
  }

  for (const fallbackLocale of localeMeta[locale].fallback) {
    const candidate = dictionary[fallbackLocale];
    if (candidate !== undefined) {
      return candidate;
    }
  }

  return dictionary.en;
}
