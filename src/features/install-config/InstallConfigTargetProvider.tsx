import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { InstallConfigOverlay } from "./InstallConfigOverlay";

/*
 * 「安装配置」的打开状态（`#354` 切片 D4）。
 *
 * 覆盖层由本 Provider 自己渲染，而不是让每个入口各挂一份：入口会越来越多（Mod 卡片右键、
 * 详情对话框、将来的批量），各挂一份就会出现「两个入口同时开着两个面板」这种没人想要的
 * 状态。目标只有一个，面板就只有一个。
 *
 * 目标不落盘、不进路由——它是一次会话内的事务状态，关掉就该没了。
 */

export type InstallConfigTarget = {
  modId: string;
  /** 只用于面板标题，不参与任何后端调用。 */
  modName: string;
};

type InstallConfigTargetContextValue = {
  target: InstallConfigTarget | null;
  openInstallConfig: (target: InstallConfigTarget) => void;
  closeInstallConfig: () => void;
};

const InstallConfigTargetContext = createContext<InstallConfigTargetContextValue | null>(null);

type InstallConfigTargetProviderProps = {
  children: ReactNode;
};

export function InstallConfigTargetProvider({ children }: InstallConfigTargetProviderProps) {
  const [target, setTarget] = useState<InstallConfigTarget | null>(null);

  const openInstallConfig = useCallback((next: InstallConfigTarget) => {
    setTarget(next);
  }, []);

  const closeInstallConfig = useCallback(() => {
    setTarget(null);
  }, []);

  const value = useMemo<InstallConfigTargetContextValue>(
    () => ({ target, openInstallConfig, closeInstallConfig }),
    [closeInstallConfig, openInstallConfig, target],
  );

  return (
    <InstallConfigTargetContext.Provider value={value}>
      {children}
      {target ? <InstallConfigOverlay target={target} onClose={closeInstallConfig} /> : null}
    </InstallConfigTargetContext.Provider>
  );
}

export function useInstallConfigTarget(): InstallConfigTargetContextValue {
  const context = useContext(InstallConfigTargetContext);

  if (!context) {
    throw new Error("useInstallConfigTarget must be used inside InstallConfigTargetProvider.");
  }

  return context;
}
