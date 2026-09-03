// #286 3b-2「A+」：外部状态扫描结果的会话表——**纯逻辑，不含 React**。
//
// 方案 A 把这张表放在 mod 库页面的 state 里，路由切换会卸载页面、表随之清空，
// 卡片徽标跟着消失（拍板时写明的代价，维护者验收时被它绊了一下，改为 A+）。
// A+ 只改「表放在哪」：提到应用级 Provider，路由切换不再丢；语义不变——
// 只存内存、不落盘、不主动失效，仍只覆盖「本会话扫过的 MOD」，与后端进程内缓存对齐。
//
// 表按 (gameId, profileId) 作用域记账：后端缓存键就是这三元组 (game, profile, mod)，
// 切换任一项，旧结果对新作用域都不成立——旧实现是「切配置档整表清空」，这里等价地
// 表述为「记录到不同作用域时整表换新；读取不同作用域时视为空表」。

import type { ExternalModStateDto } from "./externalStateApi";

export type ExternalStateSessionScope = {
  gameId: string;
  profileId: string;
};

export type ExternalStateSession = {
  /** 表里的结果属于哪个作用域；从未记录过时为 null。 */
  scope: ExternalStateSessionScope | null;
  results: ReadonlyMap<string, ExternalModStateDto>;
};

const EMPTY_RESULTS: ReadonlyMap<string, ExternalModStateDto> = new Map();

export const EMPTY_EXTERNAL_STATE_SESSION: ExternalStateSession = {
  scope: null,
  results: EMPTY_RESULTS,
};

export function sameExternalStateScope(
  left: ExternalStateSessionScope,
  right: ExternalStateSessionScope,
): boolean {
  return left.gameId === right.gameId && left.profileId === right.profileId;
}

/**
 * 记录一条 getter 结果。作用域与表当前作用域不同（切了配置档）时整表换新——
 * 旧配置档的结果对新配置档不成立，留着只会让卡片顶着别的配置档的徽标。
 */
export function recordExternalStateResult(
  session: ExternalStateSession,
  scope: ExternalStateSessionScope,
  modId: string,
  state: ExternalModStateDto,
): ExternalStateSession {
  const base =
    session.scope !== null && sameExternalStateScope(session.scope, scope)
      ? session.results
      : EMPTY_RESULTS;
  const results = new Map(base);
  results.set(modId, state);
  return { scope, results };
}

/**
 * 供某个作用域读取的结果表。没有作用域（配置档未就绪）或表属于别的作用域时为空表——
 * 卡片据此回落到清单里的托管状态，绝不显示别的配置档的徽标。
 */
export function externalStateResultsForScope(
  session: ExternalStateSession,
  scope: ExternalStateSessionScope | null,
): ReadonlyMap<string, ExternalModStateDto> {
  if (scope === null || session.scope === null || !sameExternalStateScope(session.scope, scope)) {
    return EMPTY_RESULTS;
  }
  return session.results;
}
