import type { GameId, GameSetupErrorCode } from "./gameSetupTypes";

export type GamePrerequisiteReportState =
  | "not_configured"
  | "game_directory_invalid"
  | "game_directory_not_writable"
  | "rules_unavailable"
  | "ready";

export type GamePrerequisiteSummaryStatus = "verified" | "warning" | "error";

export type GamePrerequisiteItemStatus =
  | "missing"
  | "misconfigured"
  | "installed_verified"
  | "installed_unverified";

export type GamePrerequisiteIssueCode =
  | "missing_required_file"
  | "signature_unverified"
  | "config_read_failed"
  | "config_invalid_json"
  | "config_field_mismatch"
  | "rules_unavailable"
  | "rules_corrupted";

export type GamePrerequisiteIssueDto = {
  code: GamePrerequisiteIssueCode;
  path: string;
};

export type GamePrerequisiteItemDto = {
  id: string;
  displayName: string;
  status: GamePrerequisiteItemStatus;
  issues: GamePrerequisiteIssueDto[];
};

export type GamePrerequisiteReportDto = {
  gameId: string;
  state: GamePrerequisiteReportState;
  summaryStatus: GamePrerequisiteSummaryStatus | null;
  items: GamePrerequisiteItemDto[];
  errorCode: GameSetupErrorCode | null;
  message: string | null;
};

export type GamePrerequisiteLoadState =
  | { status: "loading" }
  | { status: "not_configured" }
  | { status: "game_directory_invalid"; errorCode: GameSetupErrorCode; message: string | null }
  | { status: "game_directory_not_writable"; message: string | null }
  | { status: "rules_unavailable"; errorCode: GameSetupErrorCode; message: string | null }
  | {
      status: "ready";
      gameId: GameId;
      summaryStatus: GamePrerequisiteSummaryStatus;
      items: GamePrerequisiteItemDto[];
    };
