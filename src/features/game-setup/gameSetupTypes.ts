export type GameId = "mhw";

export type GameSetupErrorCode =
  | "unsupported_game"
  | "directory_not_found"
  | "missing_executable"
  | "storage_failed"
  | "storage_corrupted"
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

export type CommandErrorDto = {
  code: GameSetupErrorCode;
  message: string;
};

export type GameSetupStatus =
  | { kind: "not_configured"; gameId: GameId }
  | { kind: "validating"; gameId: GameId }
  | { kind: "invalid"; gameId: GameId; errorCode: GameSetupErrorCode; message: string }
  | { kind: "configured"; gameId: GameId; displayName: string; pathLabel: string };
