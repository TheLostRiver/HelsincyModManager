import type { TaskProgressEventDto } from "./modImportTypes";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import type { ModRevisionList } from "./modLibraryTypes";
import type { ModReinstallCopy } from "./modReinstallCopy";
import type { ReinstallBlockingReason, ReinstallPlanPreview } from "./modReinstallTypes";

export type ReinstallTaskPhase =
  | "install.reinstall.queued"
  | "install.reinstall.plan.building"
  | "install.reinstall.preflight.processing"
  | "install.reinstall.commit.processing"
  | "install.reinstall.rollback.processing"
  | "install.reinstall.completed"
  | "install.reinstall.failed"
  | "install.reinstall.cancelled";

export type ReinstallFailurePhase =
  | "planning"
  | "preflight"
  | "lock"
  | "backup"
  | "commit"
  | "manifest"
  | "post_commit"
  | "rollback"
  | "complete";

type ReinstallTaskIdentity = {
  taskId: string;
  modId: string;
  modName: string;
  candidateRevisionId: string;
};

export type ReinstallTaskState =
  | { status: "idle" }
  | { status: "starting"; modId: string; modName: string; candidateRevisionId: string }
  | (ReinstallTaskIdentity & { status: "running"; phase: ReinstallTaskPhase })
  | (ReinstallTaskIdentity & { status: "completed"; phase: "install.reinstall.completed" })
  | (ReinstallTaskIdentity & { status: "cancelled"; phase: "install.reinstall.cancelled" })
  | (ReinstallTaskIdentity & {
      status: "failed";
      phase: "install.reinstall.failed";
      failurePhase: ReinstallFailurePhase | null;
      message: string;
    });

// 阶段/失败段集合只承担语义判断；文本一律经 modReinstallCopy.task 取。
const reinstallTaskPhases: ReadonlySet<string> = new Set([
  "install.reinstall.queued",
  "install.reinstall.plan.building",
  "install.reinstall.preflight.processing",
  "install.reinstall.commit.processing",
  "install.reinstall.rollback.processing",
  "install.reinstall.completed",
  "install.reinstall.failed",
  "install.reinstall.cancelled",
]);

const reinstallFailurePhases: ReadonlySet<string> = new Set([
  "planning",
  "preflight",
  "lock",
  "backup",
  "commit",
  "manifest",
  "post_commit",
  "rollback",
  "complete",
]);

function commandErrorCode(error: unknown) {
  return typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string"
    ? error.code
    : null;
}

export function getReinstallPreviewErrorMessage(error: unknown, task: ModReinstallCopy["task"]) {
  switch (commandErrorCode(error)) {
    case "game_id_invalid":
      return task.previewErrors.gameUnsupported;
    case "profile_id_empty":
    case "mod_id_empty":
    case "candidate_revision_id_empty":
    case "layer_name_empty":
      return task.previewErrors.requestInvalid;
    default:
      return task.previewErrors.unavailable;
  }
}

export function getReinstallStartErrorMessage(error: unknown, task: ModReinstallCopy["task"]) {
  return commandErrorCode(error) === "plan_token_invalid"
    ? task.startErrors.planTokenInvalid
    : task.startErrors.startFailed;
}

export function isReinstallTaskPhase(phase: string): phase is ReinstallTaskPhase {
  return reinstallTaskPhases.has(phase);
}

export function getReinstallTaskPhaseLabel(phase: ReinstallTaskPhase, task: ModReinstallCopy["task"]) {
  return task.phases[phase];
}

export function getReinstallBlockingReasonLabel(
  reason: ReinstallBlockingReason,
  task: ModReinstallCopy["task"],
) {
  return task.blockingReasons[reason];
}

function parseReinstallFailurePhase(error: string | null): ReinstallFailurePhase | null {
  const prefix = "install_reinstall_failed:";
  if (!error?.startsWith(prefix)) {
    return null;
  }

  const phase = error.slice(prefix.length) as ReinstallFailurePhase;
  return reinstallFailurePhases.has(phase) ? phase : null;
}

export function nextReinstallTaskStateFromProgress(
  current: ReinstallTaskState,
  event: TaskProgressEventDto,
  task: ModReinstallCopy["task"],
): ReinstallTaskState {
  if (
    event.kind !== "install" ||
    !("taskId" in current) ||
    current.taskId !== event.taskId ||
    !isReinstallTaskPhase(event.phase)
  ) {
    return current;
  }

  const identity: ReinstallTaskIdentity = {
    taskId: current.taskId,
    modId: current.modId,
    modName: current.modName,
    candidateRevisionId: current.candidateRevisionId,
  };

  if (event.status === "completed" && event.phase === "install.reinstall.completed") {
    return { ...identity, status: "completed", phase: event.phase };
  }

  if (event.status === "cancelled" && event.phase === "install.reinstall.cancelled") {
    return { ...identity, status: "cancelled", phase: event.phase };
  }

  if (event.status === "failed" && event.phase === "install.reinstall.failed") {
    const failurePhase = parseReinstallFailurePhase(event.error);
    return {
      ...identity,
      status: "failed",
      phase: event.phase,
      failurePhase,
      message: failurePhase === null ? task.failedFallback : task.failureMessages[failurePhase],
    };
  }

  if (event.status === "queued" || event.status === "running") {
    return { ...identity, status: "running", phase: event.phase };
  }

  return current;
}

export function isReinstallTaskTerminal(state: ReinstallTaskState) {
  return state.status === "completed" || state.status === "failed" || state.status === "cancelled";
}

export function canConfirmReinstall(
  installStatus: InstallManifestStatus,
  preview: ReinstallPlanPreview,
  task: ReinstallTaskState,
) {
  return installStatus === "installed"
    && preview.status === "ready"
    && preview.prerequisiteDecision.status !== "blocked"
    && task.status === "idle";
}

export function canPreviewReinstall(
  installStatus: InstallManifestStatus,
  candidateRevisionId: string,
  task: ReinstallTaskState,
) {
  return (
    installStatus === "installed" &&
    candidateRevisionId.length > 0 &&
    task.status !== "starting" &&
    task.status !== "running"
  );
}

type RefreshReinstallDurableFactsInput = {
  loadRevisions: () => Promise<ModRevisionList>;
  loadInstallStatus: () => Promise<InstallManifestStatus>;
};

export type ReinstallDurableFactsRefreshResult = {
  revisions: ModRevisionList | null;
  installStatus: InstallManifestStatus | null;
  status: "complete" | "partial" | "failed";
};

export async function refreshReinstallDurableFacts(
  input: RefreshReinstallDurableFactsInput,
): Promise<ReinstallDurableFactsRefreshResult> {
  const [revisionsResult, installStatusResult] = await Promise.allSettled([
    input.loadRevisions(),
    input.loadInstallStatus(),
  ]);
  const revisions = revisionsResult.status === "fulfilled" ? revisionsResult.value : null;
  const installStatus = installStatusResult.status === "fulfilled" ? installStatusResult.value : null;

  return {
    revisions,
    installStatus,
    status:
      revisions !== null && installStatus !== null
        ? "complete"
        : revisions !== null || installStatus !== null
          ? "partial"
          : "failed",
  };
}
