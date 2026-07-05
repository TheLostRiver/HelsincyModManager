import { invoke } from "@tauri-apps/api/core";
import type { GameId } from "./gameSetupTypes";
import type { GamePrerequisiteReportDto } from "./gamePrerequisiteTypes";

export async function getGamePrerequisiteStatus(gameId: GameId): Promise<GamePrerequisiteReportDto> {
  return invoke<GamePrerequisiteReportDto>("get_game_prerequisite_status", { gameId });
}
