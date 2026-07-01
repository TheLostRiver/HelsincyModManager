import type { GameId } from "../game-setup/gameSetupTypes";

export type GameLaunchMethod = "steam_protocol" | "direct_executable";

export type GameLaunchReceiptDto = {
  gameId: GameId;
  method: GameLaunchMethod;
};

export type GameLaunchErrorCode =
  | "unsupported_game"
  | "game_not_configured"
  | "storage_corrupted"
  | "storage_failed"
  | "launcher_unavailable"
  | "launch_failed"
  | "unknown";

export type GameLaunchCommandErrorDto = {
  code: GameLaunchErrorCode;
  message: string;
};
