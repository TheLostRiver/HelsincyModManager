import { useCallback, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { launchGame } from "./gameLaunchApi";
import type {
  GameLaunchCommandErrorDto,
  GameLaunchErrorCode,
  GameLaunchReceiptDto,
} from "./gameLaunchTypes";

const GAME_LAUNCH_ERROR_CODES = [
  "unsupported_game",
  "game_not_configured",
  "storage_corrupted",
  "storage_failed",
  "launcher_unavailable",
  "launch_failed",
  "unknown",
] as const satisfies readonly GameLaunchErrorCode[];

type GameLaunchState = {
  isLaunchingGame: boolean;
  receipt: GameLaunchReceiptDto | null;
  errorCode: GameLaunchErrorCode | null;
  message: string | null;
};

export function useGameLaunch(gameId: GameId) {
  const [state, setState] = useState<GameLaunchState>({
    isLaunchingGame: false,
    receipt: null,
    errorCode: null,
    message: null,
  });

  const requestLaunchGame = useCallback(async () => {
    setState((current) => ({
      ...current,
      isLaunchingGame: true,
      errorCode: null,
      message: null,
    }));

    try {
      const receipt = await launchGame(gameId);
      setState({
        isLaunchingGame: false,
        receipt,
        errorCode: null,
        message: "启动请求已发送。",
      });
      return receipt;
    } catch (error) {
      const mapped = mapGameLaunchError(error);
      setState((current) => ({
        ...current,
        isLaunchingGame: false,
        errorCode: mapped.code,
        message: messageForGameLaunchError(mapped.code),
      }));
      return null;
    }
  }, [gameId]);

  return {
    isLaunchingGame: state.isLaunchingGame,
    gameLaunchReceipt: state.receipt,
    gameLaunchErrorCode: state.errorCode,
    gameLaunchMessage: state.message,
    launchGame: requestLaunchGame,
  };
}

function mapGameLaunchError(error: unknown): GameLaunchCommandErrorDto {
  if (isGameLaunchCommandErrorDto(error)) {
    return error;
  }

  return {
    code: "unknown",
    message: "启动游戏失败，请稍后重试。",
  };
}

function messageForGameLaunchError(code: GameLaunchErrorCode): string {
  switch (code) {
    case "unsupported_game":
      return "当前版本暂不支持启动该游戏。";
    case "game_not_configured":
      return "请先配置游戏目录，再启动游戏。";
    case "storage_corrupted":
      return "游戏配置文件已损坏，无法读取启动配置。";
    case "storage_failed":
      return "游戏配置读取失败，请检查应用数据目录权限。";
    case "launcher_unavailable":
      return "系统未能打开游戏启动器。";
    case "launch_failed":
      return "启动请求发送失败，请稍后重试。";
    case "unknown":
      return "启动游戏失败，请稍后重试。";
  }
}

function isGameLaunchCommandErrorDto(value: unknown): value is GameLaunchCommandErrorDto {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as { code?: unknown; message?: unknown };

  return isGameLaunchErrorCode(candidate.code) && typeof candidate.message === "string";
}

function isGameLaunchErrorCode(value: unknown): value is GameLaunchErrorCode {
  return typeof value === "string" && GAME_LAUNCH_ERROR_CODES.includes(value as GameLaunchErrorCode);
}
