import { useCallback, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { launchGame } from "./gameLaunchApi";
import type { GameLaunchCopy } from "./gameLaunchCopy";
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

// state 只存语义 outcome/errorCode，展示文本在渲染时经 gameLaunchCopy 取。
export type GameLaunchOutcome = "sent" | "failed";

type GameLaunchState = {
  isLaunchingGame: boolean;
  receipt: GameLaunchReceiptDto | null;
  errorCode: GameLaunchErrorCode | null;
  outcome: GameLaunchOutcome | null;
};

export function useGameLaunch(gameId: GameId) {
  const [state, setState] = useState<GameLaunchState>({
    isLaunchingGame: false,
    receipt: null,
    errorCode: null,
    outcome: null,
  });

  const requestLaunchGame = useCallback(async () => {
    setState((current) => ({
      ...current,
      isLaunchingGame: true,
      errorCode: null,
      outcome: null,
    }));

    try {
      const receipt = await launchGame(gameId);
      setState({
        isLaunchingGame: false,
        receipt,
        errorCode: null,
        outcome: "sent",
      });
      return receipt;
    } catch (error) {
      const mapped = mapGameLaunchError(error);
      setState((current) => ({
        ...current,
        isLaunchingGame: false,
        errorCode: mapped.code,
        outcome: "failed",
      }));
      return null;
    }
  }, [gameId]);

  return {
    isLaunchingGame: state.isLaunchingGame,
    gameLaunchReceipt: state.receipt,
    gameLaunchErrorCode: state.errorCode,
    gameLaunchOutcome: state.outcome,
    launchGame: requestLaunchGame,
  };
}

export function messageForGameLaunchOutcome(
  outcome: GameLaunchOutcome | null,
  errorCode: GameLaunchErrorCode | null,
  copy: GameLaunchCopy,
): string | null {
  if (outcome === "sent") {
    return copy.requestSent;
  }
  if (outcome === "failed") {
    return copy.errors[errorCode ?? "unknown"];
  }
  return null;
}

function mapGameLaunchError(error: unknown): GameLaunchCommandErrorDto {
  if (isGameLaunchCommandErrorDto(error)) {
    return error;
  }

  return {
    code: "unknown",
    message: "",
  };
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
