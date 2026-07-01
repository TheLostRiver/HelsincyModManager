import { invoke } from "@tauri-apps/api/core";
import type { GameId } from "../game-setup/gameSetupTypes";
import type { GameLaunchReceiptDto } from "./gameLaunchTypes";

export async function launchGame(gameId: GameId): Promise<GameLaunchReceiptDto> {
  return invoke<GameLaunchReceiptDto>("launch_game", { gameId });
}
