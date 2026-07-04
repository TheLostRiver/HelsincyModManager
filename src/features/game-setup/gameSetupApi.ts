import { invoke } from "@tauri-apps/api/core";
import type {
  GameAutoDetectionDto,
  GameCandidateScanDto,
  GameDirectoryValidationDto,
  GameId,
  GameSetupStatusDto,
} from "./gameSetupTypes";

export async function getGameSetupStatus(gameId: GameId): Promise<GameSetupStatusDto> {
  return invoke<GameSetupStatusDto>("get_game_setup_status", { gameId });
}

export async function autoDetectGameDirectory(gameId: GameId): Promise<GameAutoDetectionDto> {
  return invoke<GameAutoDetectionDto>("auto_detect_game_directory", { gameId });
}

export async function validateGameDirectory(gameId: GameId, directory: string): Promise<GameDirectoryValidationDto> {
  return invoke<GameDirectoryValidationDto>("validate_game_directory", { gameId, directory });
}

export async function saveGameDirectory(gameId: GameId, directory: string): Promise<GameSetupStatusDto> {
  return invoke<GameSetupStatusDto>("save_game_directory", { gameId, directory });
}

export async function scanGameCandidates(gameId: GameId): Promise<GameCandidateScanDto> {
  return invoke<GameCandidateScanDto>("scan_game_candidates", { gameId });
}
