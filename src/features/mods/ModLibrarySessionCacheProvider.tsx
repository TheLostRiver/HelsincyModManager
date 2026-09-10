// 库页会话缓存的应用级宿主。
//
// 挂在 RouterOutlet 之上（App.tsx），理由与 ExternalStateSessionProvider（#286）、
// ModStorageSettingsProvider（#275）、InstallConfigTargetProvider（#354 D4）完全一样：
// 路由切换会卸载页面组件，页级 state 活不过一次切页。库页的查询结果原本就是页级 state，
// 于是每次进 Mod 库都从零重查一遍，先给一屏骨架屏。
//
// 缓存本身放在 ref 上而不是 state 上：写缓存**不该**引起任何重渲染。它不是界面数据源，
// 只是「上次提交过的结果」的存放处；真正驱动界面的仍然是页面自己的查询 state。
// 顺带的好处是这里导出的几个回调身份恒定，可以安全地被下游放进 effect 依赖里。
//
// 缓存语义（命中不取消请求、按 (配置档, 查询) 分槽、失效倒向重新取数）在
// `modLibrarySessionCache.ts` 与 `modLibrarySessionStore.ts` 里；这里只持有 store 并订阅任务边界。

import { listen } from "@tauri-apps/api/event";
import { createContext, useContext, useEffect, useRef, type ReactNode } from "react";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { createModLibrarySessionStore, type ModLibrarySessionStore } from "./modLibrarySessionStore";

export type ModLibrarySessionCacheValue = ModLibrarySessionStore;

const ModLibrarySessionCacheContext = createContext<ModLibrarySessionCacheValue | null>(null);

export function ModLibrarySessionCacheProvider({ children }: { children: ReactNode }) {
  const cacheRef = useRef<ModLibrarySessionStore | null>(null);
  if (cacheRef.current === null) cacheRef.current = createModLibrarySessionStore();
  const value = cacheRef.current;

  // Task completion must invalidate snapshots even after the owning page unmounts.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, ({ payload }) => {
      if (!disposed) value.observeTask(payload);
    }).then((dispose) => {
      if (disposed) { dispose(); return; }
      unlisten = dispose;
      value.setAvailable(true);
    }).catch(() => { if (!disposed) value.setAvailable(false); });
    return () => {
      disposed = true;
      unlisten?.();
      value.setAvailable(false);
    };
  }, [value]);

  return (
    <ModLibrarySessionCacheContext.Provider value={value}>
      {children}
    </ModLibrarySessionCacheContext.Provider>
  );
}

export function useModLibrarySessionCache(): ModLibrarySessionCacheValue {
  const context = useContext(ModLibrarySessionCacheContext);
  if (!context) {
    throw new Error(
      "useModLibrarySessionCache must be used inside ModLibrarySessionCacheProvider.",
    );
  }
  return context;
}
