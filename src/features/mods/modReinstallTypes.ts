import type { GameId } from "../game-setup/gameSetupTypes";
import type { ModRevisionSummary } from "./modLibraryTypes";

export type ReinstallFileLayer = {
  name: string;
  priority: number;
};

export type PreviewReinstallPlanInput = {
  gameId: GameId;
  profileId: string;
  modId: string;
  candidateRevisionId: string;
  layer: ReinstallFileLayer;
};

export type StartReinstallTaskInput = PreviewReinstallPlanInput & {
  planToken: string;
};

export type ReinstallTargetCounts = {
  retained: number;
  replaced: number;
  added: number;
  stale: number;
};

export type ReinstallBlockingReason =
  | "not_installed"
  | "candidate_not_found"
  | "candidate_not_ready"
  | "candidate_owner_mismatch"
  | "candidate_already_installed"
  | "manifest_state_unsafe"
  | "installed_revision_unknown"
  | "source_unavailable"
  | "target_missing"
  | "target_changed"
  | "target_read_failed"
  | "backup_missing"
  | "backup_read_failed"
  | "plan_conflict"
  | "cross_mod_target_conflict"
  | "preview_stale";

export type ReinstallBlockingReasonSummary = {
  code: ReinstallBlockingReason;
  count: number;
};

export type ReinstallPlanPreview =
  | {
      status: "ready";
      planToken: string;
      installedRevision: ModRevisionSummary;
      candidateRevision: ModRevisionSummary;
      counts: ReinstallTargetCounts;
      blockingReasons: [];
    }
  | {
      status: "blocked";
      planToken: null;
      installedRevision: ModRevisionSummary | null;
      candidateRevision: ModRevisionSummary | null;
      counts: ReinstallTargetCounts;
      blockingReasons: ReinstallBlockingReasonSummary[];
    };
