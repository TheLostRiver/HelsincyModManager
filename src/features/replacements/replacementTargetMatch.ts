// #274：替换目标列表的「命中提示」。
//
// 列表过滤会匹配全部语言的展示名、全部别名和内部 ID，但一行只渲染当前语言展示名、
// 英文副名和内部 ID。当关键词命中的是没渲染出来的名字——共用同一模型的其他强化
// 阶段（别名），或另一种语言的展示名——这一行看起来就像搜错了。这里算出应当补充
// 显示的「匹配：…」内容。纯函数、零运行时依赖，node --test 可直接加载。

export type ReplacementTargetSearchableNames = {
  displayNames: Record<string, string>;
  aliases: string[];
};

export type ReplacementTargetRenderedNames = {
  displayName: string;
  secondaryName?: string;
};

export type ReplacementTargetMatchHint = {
  /** 命中但未渲染的名字，最多 `limit` 个，展示名（其他语言）在前、别名在后。 */
  names: string[];
  /** 被截掉的命中数。 */
  hiddenCount: number;
};

export const REPLACEMENT_TARGET_MATCH_HINT_LIMIT = 2;

/** 与列表过滤完全相同的命中判据：大小写不敏感的子串匹配；`keyword` 需已 trim + 小写。 */
export function replacementTargetSearchHit(value: string, keyword: string): boolean {
  return value.toLocaleLowerCase().includes(keyword);
}

export function matchedHiddenReplacementTargetNames(
  target: ReplacementTargetSearchableNames,
  rendered: ReplacementTargetRenderedNames,
  query: string,
  limit: number = REPLACEMENT_TARGET_MATCH_HINT_LIMIT,
): ReplacementTargetMatchHint | null {
  const keyword = query.trim().toLocaleLowerCase();
  if (!keyword) {
    return null;
  }
  const visible = [rendered.displayName, rendered.secondaryName].filter(
    (value): value is string => Boolean(value),
  );
  // 行里已经能看到命中的名字时不再重复提示，避免每一行都多一行噪音。
  if (visible.some((value) => replacementTargetSearchHit(value, keyword))) {
    return null;
  }
  const visibleSet = new Set(visible);
  const seen = new Set<string>();
  const matched: string[] = [];
  for (const value of [...Object.values(target.displayNames), ...target.aliases]) {
    if (!value || visibleSet.has(value) || seen.has(value)) {
      continue;
    }
    seen.add(value);
    if (replacementTargetSearchHit(value, keyword)) {
      matched.push(value);
    }
  }
  if (matched.length === 0) {
    return null;
  }
  return {
    names: matched.slice(0, limit),
    hiddenCount: Math.max(0, matched.length - limit),
  };
}
