export type GameId = "mhw";

export type GameSetupErrorCode =
  | "unsupported_game"
  | "directory_not_found"
  | "directory_not_absolute"
  | "missing_executable"
  | "storage_failed"
  | "storage_corrupted"
  | "scan_failed"
  | "scan_not_implemented"
  | "unknown";

export type GameSetupStatusDto = {
  gameId: string;
  kind: "not_configured" | "invalid" | "configured";
  displayName: string | null;
  pathLabel: string | null;
  errorCode: GameSetupErrorCode | null;
  message: string | null;
};

export type GameAutoDetectionOutcome =
  | "already_configured"
  | "detected_and_saved"
  | "not_found"
  | "invalid_candidate"
  | "scan_failed";

export type GameAutoDetectionDto = {
  gameId: string;
  outcome: GameAutoDetectionOutcome;
  status: GameSetupStatusDto;
  errorCode: GameSetupErrorCode | null;
  candidateCount: number;
};

export type GameDirectoryEvidenceDto = {
  kind: string;
  label: string;
};

export type GameDirectoryValidationDto = {
  gameId: string;
  isValid: boolean;
  confidence: number;
  evidence: GameDirectoryEvidenceDto[];
  errors: GameSetupErrorCode[];
  pathLabel: string;
};

export type GameCandidateSource = "steam";

export type GameDirectoryCandidateDto = {
  gameId: string;
  displayName: string;
  directory: string;
  pathLabel: string;
  source: GameCandidateSource;
  sourceLabel: string;
  isValid: boolean;
  confidence: number;
  evidence: GameDirectoryEvidenceDto[];
  errors: string[];
};

export type GameCandidateScanDto = {
  gameId: string;
  candidates: GameDirectoryCandidateDto[];
};

export type GameDirectoryCandidate = Omit<GameDirectoryCandidateDto, "errors" | "gameId"> & {
  gameId: GameId;
  errors: GameSetupErrorCode[];
};

export type CommandErrorDto = {
  code: GameSetupErrorCode;
  message: string;
};

export type GameSetupStatus =
  | { kind: "not_configured"; gameId: GameId }
  | { kind: "validating"; gameId: GameId }
  | { kind: "invalid"; gameId: GameId; errorCode: GameSetupErrorCode; backendMessage: string | null }
  | { kind: "configured"; gameId: GameId; displayName: string; pathLabel: string };

// 启动自检通知只存语义：文本在渲染时经 gameSetupCopy 取（语义/文本分离）。
export type GameSetupStartupNoticeDetailKind =
  | "invalid_candidate"
  | "not_found"
  | "startup_timeout"
  | "command_error";

export type GameSetupStartupNotice = {
  errorCode: GameSetupErrorCode;
  detailKind: GameSetupStartupNoticeDetailKind;
  /** 仅 detailKind === "command_error" 时可能存在，为后端透传消息。 */
  backendDetail: string | null;
};
