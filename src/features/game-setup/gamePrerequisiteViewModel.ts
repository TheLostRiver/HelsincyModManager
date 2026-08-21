import type { GameId, GameSetupErrorCode } from "./gameSetupTypes";
import type { GamePrerequisiteLoadState, GamePrerequisiteReportDto } from "./gamePrerequisiteTypes";

const RULES_UNAVAILABLE_FALLBACK = "前置规则暂不可用。";
const GAME_DIRECTORY_INVALID_FALLBACK = "当前保存的游戏目录已失效，请重新选择。";
const GAME_DIRECTORY_NOT_WRITABLE_FALLBACK =
  "游戏目录当前不可写。请先完全退出游戏与 Steam，确认目录未被设为只读或被安全软件占用后重试。";

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

  if (dto.state === "game_directory_not_writable") {
    // 目录结构是好的，写不进去而已——不要复用"校验失败"的文案误导用户去改目录。
    return {
      status: "game_directory_not_writable",
      message: dto.message ?? GAME_DIRECTORY_NOT_WRITABLE_FALLBACK,
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
  if (value !== "mhw") {
    throw new Error(`Unexpected gameId from backend: ${value}`);
  }

  return value;
}

function normalizeErrorCode(value: GameSetupErrorCode | null): GameSetupErrorCode {
  return value ?? "unknown";
}
