// 库页会话缓存的应用级宿主。
//
// 挂在 RouterOutlet 之上（App.tsx），理由与 ExternalStateSessionProvider（#286）、
// ModStorageSettingsProvider（#275）、InstallConfigTargetProvider（#354 D4）完全一样：
// 路由切换会卸载页面组件，页级 state 活不过一次切页。库页的查询结果原本就是页级 state，
// 于是每次进 Mod 库都从零重查一遍，先给一屏骨架屏。
//
// Store 放在 ref 上保持身份稳定。目录缓存写回不驱动查询；安装状态事件通过独立展示订阅
// 更新对应卡片，不推进查询 generation。写入边界的轻量补读负责恢复丢失的状态通知。
//
// 缓存语义（命中不取消请求、按 (配置档, 查询) 分槽、失效倒向重新取数）在
// `modLibrarySessionCache.ts` 与 `modLibrarySessionStore.ts` 里；这里只持有 store 并订阅任务边界。

import { listen } from "@tauri-apps/api/event";
import { createContext, useContext, useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { createModLibrarySessionStore, type ModLibrarySessionStore } from "./modLibrarySessionStore";
import { attachModLibraryWriteTracking, publishModLibraryTaskProgress } from "./modLibraryWriteTracking.ts";
import { getTaskProgress } from "./modTaskProgressApi.ts";
import { MOD_INSTALLATION_STATE_EVENT, type ModInstallationStateEvent } from "./modInstallationStateTypes";

export type ModLibrarySessionCacheValue = ModLibrarySessionStore;

const ModLibrarySessionCacheContext = createContext<ModLibrarySessionCacheValue | null>(null);

export function ModLibrarySessionCacheProvider({ children }: { children: ReactNode }) {
  const cacheRef = useRef<ModLibrarySessionStore | null>(null);
  if (cacheRef.current === null) cacheRef.current = createModLibrarySessionStore();
  const value = cacheRef.current;
  useLayoutEffect(() => attachModLibraryWriteTracking(value), [value]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ModInstallationStateEvent>(MOD_INSTALLATION_STATE_EVENT, ({ payload }) => {
      if (!disposed) value.observeInstallationState(payload);
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    }).catch(() => { /* Terminal metadata queries still reconcile all retained cards. */ });
    return () => { disposed = true; unlisten?.(); };
  }, [value]);

  // Events are the fast path. Poll only active desktop tasks to recover a missed terminal
  // event, even after the page unmounts. An unavailable/unknown task stays occupied.
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      await Promise.all(value.activeTaskIds().map(async (taskId) => {
        try {
          const event = await getTaskProgress(taskId);
          if (!disposed && event?.taskId === taskId) publishModLibraryTaskProgress(event);
        } catch { /* Keep the write occupied; the next observation can recover it. */ }
      }));
      if (!disposed) timer = setTimeout(() => { void poll(); }, 1500);
    };
    timer = setTimeout(() => { void poll(); }, 1500);
    return () => { disposed = true; clearTimeout(timer); };
  }, [value]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("mod-library-statistics-updated", () => {
      if (!disposed) value.invalidateAllPages();
    }).then((dispose) => {
      if (disposed) { dispose(); return; }
      unlisten = dispose;
      // Also covers a startup refresh that completed before the listener was ready.
      value.invalidateAllPages();
    }).catch(() => {});
    return () => { disposed = true; unlisten?.(); };
  }, [value]);

  // Task completion must invalidate snapshots even after the owning page unmounts.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, ({ payload }) => {
      if (!disposed) publishModLibraryTaskProgress(payload);
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
