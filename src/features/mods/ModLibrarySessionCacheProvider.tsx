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
// `modLibrarySessionCache.ts` 里，纯逻辑可单测；这里只负责持有与派发。

import { createContext, useContext, useMemo, useRef, type ReactNode } from "react";
import type { CategoryItem } from "./modCategoryApi";
import type { ModLibraryPage } from "./modLibraryTypes";
import {
  EMPTY_MOD_LIBRARY_SESSION_CACHE,
  invalidateAllCachedLibraryPages,
  invalidateCachedLibraryPage,
  readCachedCategories,
  readCachedLibraryPage,
  writeCachedCategories,
  writeCachedLibraryPage,
  type ModLibrarySessionCache,
} from "./modLibrarySessionCache";

export type ModLibrarySessionCacheValue = {
  readPage: (profileKey: string, queryKey: string) => ModLibraryPage | null;
  writePage: (profileKey: string, queryKey: string, page: ModLibraryPage) => void;
  invalidatePage: (profileKey: string, queryKey: string) => void;
  /** 库变了但本页没经手时调用（后台导入完成、写任务开跑）。 */
  invalidateAllPages: () => void;
  readCategories: () => readonly CategoryItem[] | null;
  writeCategories: (categories: readonly CategoryItem[]) => void;
};

const ModLibrarySessionCacheContext = createContext<ModLibrarySessionCacheValue | null>(null);

export function ModLibrarySessionCacheProvider({ children }: { children: ReactNode }) {
  const cacheRef = useRef<ModLibrarySessionCache>(EMPTY_MOD_LIBRARY_SESSION_CACHE);
  const value = useMemo<ModLibrarySessionCacheValue>(
    () => ({
      readPage: (profileKey, queryKey) =>
        readCachedLibraryPage(cacheRef.current, profileKey, queryKey),
      writePage: (profileKey, queryKey, page) => {
        cacheRef.current = writeCachedLibraryPage(cacheRef.current, profileKey, queryKey, page);
      },
      invalidatePage: (profileKey, queryKey) => {
        cacheRef.current = invalidateCachedLibraryPage(cacheRef.current, profileKey, queryKey);
      },
      invalidateAllPages: () => {
        cacheRef.current = invalidateAllCachedLibraryPages(cacheRef.current);
      },
      readCategories: () => readCachedCategories(cacheRef.current),
      writeCategories: (categories) => {
        cacheRef.current = writeCachedCategories(cacheRef.current, categories);
      },
    }),
    [],
  );

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
