import { localeMeta, type Locale } from "../../shared/i18n";

// I18N-08 契约：DTO 携带全语言 displayNames（键集即 per-game 名称 locale 能力声明），
// 展示名在渲染时按共享 fallback 链（locale → fallback 链 → en → 任一可用）投影，
// 语言切换不重拉目标列表。次要名固定取 en（与主名不同时显示，沿用既有 zh 行为）。

export type ReplacementTargetDisplayNames = Record<string, string>;

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

/** 跨语言检索语义：任一语言的展示名都参与匹配，不随界面语言变化。 */
export function replacementTargetSearchValues(displayNames: ReplacementTargetDisplayNames): string[] {
  return Object.values(displayNames);
}
