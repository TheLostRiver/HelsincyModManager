import type { Locale } from "../../shared/i18n/locales.ts";
import type { ReplacementTarget } from "./replacementTypes.ts";
import { replacementTargetSearchHit } from "./replacementTargetMatch.ts";
import { resolveReplacementTargetAliases, resolveReplacementTargetNames } from "./replacementTargetNames.ts";

export type ReplacementTargetOption = {
  /** 仅用于列表与 radio；执行请求始终使用 target.id。 */
  key: string;
  target: ReplacementTarget;
  alias: string | null;
  displayName: string;
  secondaryName?: string;
};

export function replacementTargetOption(
  target: ReplacementTarget,
  locale: Locale,
  alias: string | null = null,
): ReplacementTargetOption {
  // 平表由后端验证过。保留所选的原文名称，不把不同语言独立排序的别名按下标配对。
  const selectedAlias = target.targetType === "weapon" && alias !== null && target.aliases.includes(alias) ? alias : null;
  const names = selectedAlias === null
    ? resolveReplacementTargetNames(target.displayNames, locale)
    : { displayName: selectedAlias };
  return { key: JSON.stringify([target.id, selectedAlias]), target, alias: selectedAlias, ...names };
}

function visibleTargetOptions(target: ReplacementTarget, locale: Locale): ReplacementTargetOption[] {
  const primary = replacementTargetOption(target, locale);
  if (target.targetType !== "weapon") return [primary];
  const seen = new Set([primary.displayName]);
  const aliases = resolveReplacementTargetAliases(target.aliasesByLocale, locale).filter((name) => {
    if (!name.trim() || seen.has(name) || !target.aliases.includes(name)) return false;
    seen.add(name);
    return true;
  });
  return [primary, ...aliases.map((alias) => replacementTargetOption(target, locale, alias))];
}

/** 每个武器名称一行；每一行仍引用后端给出的同一个模型目标。防具保持一目标一行。 */
export function buildReplacementTargetOptions(
  targets: readonly ReplacementTarget[],
  locale: Locale,
  query = "",
): ReplacementTargetOption[] {
  const keyword = query.trim().toLocaleLowerCase();
  return targets.flatMap((target) => {
    const visible = visibleTargetOptions(target, locale);
    if (!keyword) return visible;
    const matches = (text: string) => replacementTargetSearchHit(text, keyword);
    const visibleMatches = visible.filter((option) =>
      [option.displayName, option.secondaryName, target.internalId].some((value) => value && matches(value)),
    );
    const primaryMatches = Object.values(target.displayNames).some(matches);
    const matchingAliases = [...new Set(target.aliases.filter(matches))];
    if (target.targetType !== "weapon") {
      return visibleMatches.length > 0 || primaryMatches || matchingAliases.length > 0 ? visible : [];
    }

    // 不知道不同语言别名之间的配对关系时，直接显示命中的原文名称。
    // 去重使用视图 key，避免当前语言的命中再从跨语言平表添加一次。
    const allMatches = [
      ...visibleMatches,
      ...(primaryMatches ? [visible[0]] : []),
      ...matchingAliases.map((alias) => replacementTargetOption(target, locale, alias)),
    ];
    return [...new Map(allMatches.map((option) => [option.key, option])).values()];
  });
}
