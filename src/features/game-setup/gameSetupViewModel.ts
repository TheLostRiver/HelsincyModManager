import type { GameSetupCopy } from "./gameSetupCopy";
import type {
  CommandErrorDto,
  GameCandidateScanDto,
  GameDirectoryCandidate,
  GameId,
  GameSetupErrorCode,
  GameSetupStatus,
  GameSetupStatusDto,
} from "./gameSetupTypes";

// mapCommandError 的归一化结果：code 为稳定语义码，backendMessage 只保留
// 后端真实透传文本；合成的 unknown 不再携带本地化字符串。
export type MappedCommandError = {
  code: GameSetupErrorCode;
  backendMessage: string | null;
};

const GAME_SETUP_ERROR_CODES = [
  "unsupported_game",
  "directory_not_found",
  "directory_not_absolute",
  "missing_executable",
  "storage_failed",
  "storage_corrupted",
  "scan_failed",
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
      backendMessage: dto.message ?? null,
    };
  }

  return { kind: "not_configured", gameId };
}

export function mapCandidateScanDto(dto: GameCandidateScanDto): GameDirectoryCandidate[] {
  return dto.candidates.map((candidate) => ({
    ...candidate,
    gameId: normalizeGameId(candidate.gameId),
    errors: candidate.errors.map(normalizeGameSetupErrorCode),
  }));
}

export function mapCommandError(error: unknown): MappedCommandError {
  if (isCommandErrorDto(error)) {
    return { code: error.code, backendMessage: error.message || null };
  }

  return {
    code: "unknown",
    backendMessage: null,
  };
}

export function messageForError(code: GameSetupErrorCode, errors: GameSetupCopy["errors"]): string {
  return errors[code];
}

function normalizeGameId(value: string): GameId {
  return value === "mhw" ? "mhw" : "mhw";
}

function normalizeGameSetupErrorCode(value: string): GameSetupErrorCode {
  return isGameSetupErrorCode(value) ? value : "unknown";
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
