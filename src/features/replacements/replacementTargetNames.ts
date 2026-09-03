// 值导入指向具体文件而不是 shared/i18n 目录桶：本模块被 node --test 直载，
// node 原生 ESM 不解析目录导入（先例：externalImportHistoryModel.ts）。
import { localeMeta, type Locale } from "../../shared/i18n/locales.ts";

// I18N-08 契约：DTO 携带全语言 displayNames（键集即 per-game 名称 locale 能力声明），
// 展示名在渲染时按共享 fallback 链（locale → fallback 链 → en → 任一可用）投影，
// 语言切换不重拉目标列表。次要名固定取 en（与主名不同时显示，沿用既有 zh 行为）。

export type ReplacementTargetDisplayNames = Record<string, string>;

/** 按语言分组的别名（locale → 别名列表）；DTO 里可选，缺席表示来源不按语言给别名。 */
export type ReplacementTargetAliasesByLocale = Record<string, string[]>;

export function resolveReplacementTargetNames(
  displayNames: ReplacementTargetDisplayNames,
  locale: Locale,
): { displayName: string; secondaryName?: string } {
  const displayName =
    displayNames[locale]
    ?? localeMeta[locale].fallback.map((fallbackLocale) => displayNames[fallbackLocale]).find(Boolean)
    ?? displayNames.en
    ?? Object.values(displayNames)[0]
    ?? "";
  const secondaryName =
    displayNames.en && displayNames.en !== displayName ? displayNames.en : undefined;

  return { displayName, secondaryName };
}

/**
 * 本语言别名，沿展示名同一条 fallback 链取词（locale → fallback 链 → en）。
 * 某语言的键存在但为空表就停在那里返回空——「这个语言没有别名」是事实，不能拿英文别名顶上；
 * 整个映射缺席（铠甲 catalog）同样返回空。不做「任一可用」兜底：别名列表跨语言乱拼没有意义。
 */
export function resolveReplacementTargetAliases(
  aliasesByLocale: ReplacementTargetAliasesByLocale | undefined,
  locale: Locale,
): string[] {
  if (!aliasesByLocale) {
    return [];
  }
  for (const candidate of [locale, ...localeMeta[locale].fallback, "en"]) {
    const aliases = aliasesByLocale[candidate];
    if (aliases) {
      return aliases;
    }
  }
  return [];
}

/** 跨语言检索语义：任一语言的展示名都参与匹配，不随界面语言变化。 */
export function replacementTargetSearchValues(displayNames: ReplacementTargetDisplayNames): string[] {
  return Object.values(displayNames);
}
