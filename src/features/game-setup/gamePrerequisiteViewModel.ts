import type { GameId, GameSetupErrorCode } from "./gameSetupTypes";
import type { GamePrerequisiteLoadState, GamePrerequisiteReportDto } from "./gamePrerequisiteTypes";

const RULES_UNAVAILABLE_FALLBACK = "前置规则暂不可用。";
const GAME_DIRECTORY_INVALID_FALLBACK = "当前保存的游戏目录已失效，请重新选择。";

export function mapPrerequisiteReportDto(dto: GamePrerequisiteReportDto): GamePrerequisiteLoadState {
  if (dto.state === "not_configured") {
    return { status: "not_configured" };
  }

  if (dto.state === "game_directory_invalid") {
    return {
      status: "game_directory_invalid",
      errorCode: normalizeErrorCode(dto.errorCode),
      message: dto.message ?? GAME_DIRECTORY_INVALID_FALLBACK,
    };
  }

  if (dto.state === "rules_unavailable") {
    return {
      status: "rules_unavailable",
      errorCode: normalizeErrorCode(dto.errorCode),
      message: dto.message ?? RULES_UNAVAILABLE_FALLBACK,
    };
  }

  return {
    status: "ready",
    gameId: normalizeGameId(dto.gameId),
    summaryStatus: dto.summaryStatus ?? "verified",
    items: dto.items,
  };
}

function normalizeGameId(value: string): GameId {
  return value === "mhw" ? "mhw" : "mhw";
}

function normalizeErrorCode(value: GameSetupErrorCode | null): GameSetupErrorCode {
  return value ?? "unknown";
}
