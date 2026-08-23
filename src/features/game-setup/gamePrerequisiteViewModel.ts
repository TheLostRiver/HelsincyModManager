import type { GameId, GameSetupErrorCode } from "./gameSetupTypes";
import type { GamePrerequisiteLoadState, GamePrerequisiteReportDto } from "./gamePrerequisiteTypes";

export function mapPrerequisiteReportDto(dto: GamePrerequisiteReportDto): GamePrerequisiteLoadState {
  if (dto.state === "not_configured") {
    return { status: "not_configured" };
  }

  // 后端未给 message 时保持 null：兜底文案由面板按当前界面语言渲染
  // （gamePrerequisiteCopy.fallbackMessage），不能在映射时固化成单一语言。
  if (dto.state === "game_directory_invalid") {
    return {
      status: "game_directory_invalid",
      errorCode: normalizeErrorCode(dto.errorCode),
      message: dto.message ?? null,
    };
  }

  if (dto.state === "game_directory_not_writable") {
    // 目录结构是好的，写不进去而已——不要复用"校验失败"的文案误导用户去改目录。
    return {
      status: "game_directory_not_writable",
      message: dto.message ?? null,
    };
  }

  if (dto.state === "rules_unavailable") {
    return {
      status: "rules_unavailable",
      errorCode: normalizeErrorCode(dto.errorCode),
      message: dto.message ?? null,
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
