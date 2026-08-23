import { createContext, useContext, type ReactNode } from "react";
import type { GameId } from "./gameSetupTypes";
import { useGameSetupState } from "./useGameSetup";

type GameSetupContextValue = ReturnType<typeof useGameSetupState>;

const GameSetupContext = createContext<GameSetupContextValue | null>(null);

type GameSetupProviderProps = {
  gameId?: GameId;
  children: ReactNode;
};

/**
 * 全应用共享一份游戏目录配置状态。
 *
 * 改动前每个需要它的组件各自调用状态 hook，带来两个问题：启动自检（含 Steam 库扫描
 * 与 10 秒超时）按调用方数量重复执行；各方持有独立副本，在工作台配置完目录后
 * 顶部状态栏不会跟着更新。集中到 provider 后两者都消失。
 */
export function GameSetupProvider({ gameId, children }: GameSetupProviderProps) {
  const value = useGameSetupState(gameId);

  return <GameSetupContext.Provider value={value}>{children}</GameSetupContext.Provider>;
}

export function useGameSetup(): GameSetupContextValue {
  const value = useContext(GameSetupContext);

  if (!value) {
    throw new Error("useGameSetup must be used inside GameSetupProvider.");
  }

  return value;
}
