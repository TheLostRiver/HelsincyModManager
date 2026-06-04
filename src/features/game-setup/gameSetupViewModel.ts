import type {
  CommandErrorDto,
  GameId,
  GameSetupErrorCode,
  GameSetupStatus,
  GameSetupStatusDto,
} from "./gameSetupTypes";

const GAME_SETUP_ERROR_CODES = [
  "unsupported_game",
  "directory_not_found",
  "directory_not_absolute",
  "missing_executable",
  "storage_failed",
  "storage_corrupted",
  "scan_not_implemented",
  "unknown",
] as const satisfies readonly GameSetupErrorCode[];

export function mapStatusDto(dto: GameSetupStatusDto): GameSetupStatus {
  const gameId = normalizeGameId(dto.gameId);

  if (dto.kind === "configured") {
    return {
      kind: "configured",
      gameId,
      displayName: dto.displayName ?? "Monster Hunter: World - Iceborne",
      pathLabel: dto.pathLabel ?? ".../Monster Hunter World",
    };
  }

  if (dto.kind === "invalid") {
    return {
      kind: "invalid",
      gameId,
      errorCode: dto.errorCode ?? "unknown",
      message: dto.message ?? messageForError(dto.errorCode ?? "unknown"),
    };
  }

  return { kind: "not_configured", gameId };
}

export function mapCommandError(error: unknown): CommandErrorDto {
  if (isCommandErrorDto(error)) {
    return error;
  }

  return {
    code: "unknown",
    message: "操作失败，请稍后重试。",
  };
}

export function messageForError(code: GameSetupErrorCode): string {
  switch (code) {
    case "unsupported_game":
      return "当前版本暂不支持该游戏。";
    case "directory_not_found":
      return "所选目录不存在。";
    case "directory_not_absolute":
      return "请选择完整的游戏安装目录，不能使用相对路径。";
    case "missing_executable":
      return "所选目录缺少 MonsterHunterWorld.exe。";
    case "storage_failed":
      return "配置保存失败，请检查应用数据目录权限。";
    case "storage_corrupted":
      return "配置文件已损坏，请先处理应用数据目录中的 games.json。";
    case "scan_not_implemented":
      return "自动扫描 Steam 尚未启用，请先手动选择目录。";
    case "unknown":
      return "操作失败，请稍后重试。";
  }
}

function normalizeGameId(value: string): GameId {
  return value === "mhw" ? "mhw" : "mhw";
}

function isCommandErrorDto(value: unknown): value is CommandErrorDto {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as { code?: unknown; message?: unknown };

  return isGameSetupErrorCode(candidate.code) && typeof candidate.message === "string";
}

function isGameSetupErrorCode(value: unknown): value is GameSetupErrorCode {
  return typeof value === "string" && GAME_SETUP_ERROR_CODES.includes(value as GameSetupErrorCode);
}
