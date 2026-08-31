// 「检查更新」的策略：**纯逻辑，不碰 DOM、不碰网络**，因此可以被 node --test 直接
// 加载并单测。持久化见 `updateCheckStorage.ts`，命令调用见 `updateCheckApi.ts`。

import {
  DEFAULT_UPDATE_CHECK_PREFERENCE,
  type UpdateCheckPreference,
} from "./updateCheckTypes.ts";

/** 两次自动查询之间的最短间隔。不要每次打开页面都去打扰 GitHub。 */
export const UPDATE_CHECK_MIN_INTERVAL_MILLIS = 24 * 60 * 60 * 1000;

/**
 * 现在是否该发起查询。
 *
 * 规则（顺序即优先级）：
 *
 * 1. 关掉自动检查 → 永不查询；
 * 2. 从未查过 → 查；
 * 3. 距上次查询已超过最短间隔 → 查；
 * 4. 其余 → 不查。
 *
 * 第 3 条用 `>=` 而不是 `>`：正好间隔 24 小时算「已过期」。
 * 另外，若 `lastCheckedAt` 在 `now` **之后**（系统时钟被往回调、或手工改过存档），
 * 间隔会算成负数——这时按「该查」处理，而不是永远卡住不查。
 */
export function shouldCheckForUpdate(
  preference: UpdateCheckPreference,
  now: number,
): boolean {
  if (!preference.autoCheckEnabled) {
    return false;
  }
  if (preference.lastCheckedAt === null) {
    return true;
  }

  const elapsed = now - preference.lastCheckedAt;
  return elapsed < 0 || elapsed >= UPDATE_CHECK_MIN_INTERVAL_MILLIS;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

/**
 * 把任意来源（localStorage 里的旧数据、手工编辑过的值）收敛成合法的偏好。
 *
 * **宽容但不放任**：认不出来的字段退回默认值，而不是抛错——偏好读不出来
 * 只该让用户回到默认体验，不该让「关于」页挂掉。
 */
export function sanitizeUpdateCheckPreference(value: unknown): UpdateCheckPreference {
  if (typeof value !== "object" || value === null) {
    return DEFAULT_UPDATE_CHECK_PREFERENCE;
  }

  const record = value as Record<string, unknown>;
  const autoCheckEnabled =
    typeof record.autoCheckEnabled === "boolean"
      ? record.autoCheckEnabled
      : DEFAULT_UPDATE_CHECK_PREFERENCE.autoCheckEnabled;
  const lastCheckedAt = isFiniteNumber(record.lastCheckedAt)
    ? record.lastCheckedAt
    : DEFAULT_UPDATE_CHECK_PREFERENCE.lastCheckedAt;

  return { autoCheckEnabled, lastCheckedAt };
}
