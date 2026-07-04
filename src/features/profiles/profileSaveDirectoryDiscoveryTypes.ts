import type { ProfileDirectorySelectionDto } from "./profileSaveSettingsTypes";

export type SaveDirectoryDiscoveryOutcome =
  | "auto_saved"
  | "confirmation_required"
  | "not_found"
  | "existing_valid"
  | "existing_invalid"
  | "scan_failed";

export type SaveDirectoryCandidateDto = {
  candidateId: string;
  source: "steam_userdata";
  confidence: "high" | "medium" | "low";
  recommended: boolean;
  accountName: string | null;
  avatarUrl: string | null;
  accountLabel: string;
  pathLabel: string;
  lastModifiedAt: number | null;
  evidence: string[];
};

export type SaveDirectoryDiscoveryDto = {
  discoveryId: string;
  gameId: string;
  profileId: string;
  outcome: SaveDirectoryDiscoveryOutcome;
  recommendedCandidateId: string | null;
  candidates: SaveDirectoryCandidateDto[];
  savedSettings?: ProfileDirectorySelectionDto | null;
  errorCode?: string | null;
};

export type DiscoverProfileSaveDirectoriesInput = {
  gameId: string;
  profileId: string;
};

export type ConfirmProfileSaveDirectoryCandidateInput = {
  discoveryId: string;
  candidateId: string;
};
