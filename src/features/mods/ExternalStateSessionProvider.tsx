// #286 3b-2「A+」：外部状态扫描结果会话表的应用级宿主。
//
// 挂在 RouterOutlet 之上（App.tsx）：路由切换会卸载页面组件，页级 state 活不过一次
// 切页，而「会话级」承诺的是本次运行期间——所以表要放在页面之外。表本身的语义
// 在 `externalStateSession.ts`（纯逻辑，可单测）；这里只负责持有与派发。

import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react";
import type { ExternalModStateDto } from "./externalStateApi";
import {
  EMPTY_EXTERNAL_STATE_SESSION,
  externalStateResultsForScope,
  recordExternalStateResult,
  type ExternalStateSession,
  type ExternalStateSessionScope,
} from "./externalStateSession";

type ExternalStateSessionContextValue = {
  session: ExternalStateSession;
  setSession: (update: (previous: ExternalStateSession) => ExternalStateSession) => void;
};

const ExternalStateSessionContext = createContext<ExternalStateSessionContextValue | null>(null);

export function ExternalStateSessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<ExternalStateSession>(EMPTY_EXTERNAL_STATE_SESSION);
  const value = useMemo<ExternalStateSessionContextValue>(
    () => ({ session, setSession }),
    [session],
  );
  return (
    <ExternalStateSessionContext.Provider value={value}>
      {children}
    </ExternalStateSessionContext.Provider>
  );
}

export type ExternalStateSessionView = {
  /** 该作用域下本会话扫过的 MOD 的最新 getter 结果；作用域为 null 或不匹配时为空表。 */
  results: ReadonlyMap<string, ExternalModStateDto>;
  /** 记录一条 getter 结果到当前作用域；作用域为 null（配置档未就绪）时忽略。 */
  record: (modId: string, state: ExternalModStateDto) => void;
};

/**
 * 以某个 (game, profile) 作用域读写会话表。作用域对象每次渲染可以是新引用，
 * 内部按字段比较，不会因此重算。
 */
export function useExternalStateSession(
  scope: ExternalStateSessionScope | null,
): ExternalStateSessionView {
  const context = useContext(ExternalStateSessionContext);
  if (!context) {
    throw new Error("useExternalStateSession must be used inside ExternalStateSessionProvider.");
  }
  const { session, setSession } = context;
  const scopeRef = useRef(scope);
  scopeRef.current = scope;

  const gameId = scope?.gameId ?? null;
  const profileId = scope?.profileId ?? null;
  const results = useMemo(
    () =>
      externalStateResultsForScope(
        session,
        gameId !== null && profileId !== null ? { gameId, profileId } : null,
      ),
    [session, gameId, profileId],
  );

  const record = useCallback(
    (modId: string, state: ExternalModStateDto) => {
      const currentScope = scopeRef.current;
      if (currentScope === null) {
        return;
      }
      setSession((previous) => recordExternalStateResult(previous, currentScope, modId, state));
    },
    [setSession],
  );

  return { results, record };
}
