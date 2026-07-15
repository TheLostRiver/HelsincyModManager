import type { TaskProgressEventDto } from "./modImportTypes";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import type { ModRevisionList } from "./modLibraryTypes";
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

const reinstallTaskPhaseLabels: Record<ReinstallTaskPhase, string> = {
  "install.reinstall.queued": "等待重装",
  "install.reinstall.plan.building": "生成重装计划",
  "install.reinstall.preflight.processing": "执行提交前检查",
  "install.reinstall.commit.processing": "提交新版本",
  "install.reinstall.rollback.processing": "恢复原版本",
  "install.reinstall.completed": "重装完成",
  "install.reinstall.failed": "重装失败",
  "install.reinstall.cancelled": "重装已取消",
};

const reinstallBlockingReasonLabels: Record<ReinstallBlockingReason, string> = {
  not_installed: "当前 Mod 尚未安装",
  candidate_not_found: "候选版本不存在",
  candidate_not_ready: "候选版本尚未准备完成",
  candidate_owner_mismatch: "候选版本不属于当前 Mod",
  candidate_already_installed: "候选版本已安装",
  manifest_state_unsafe: "当前安装状态不允许重装",
  installed_revision_unknown: "无法确认当前已安装版本",
  source_unavailable: "候选版本源文件不可用",
  target_missing: "受管目标文件缺失",
  target_changed: "受管目标文件已发生变化",
  target_read_failed: "无法读取受管目标文件",
  backup_missing: "所需备份缺失",
  backup_read_failed: "无法读取所需备份",
  plan_conflict: "重装计划存在冲突",
  cross_mod_target_conflict: "与其他 Mod 的目标文件冲突",
  preview_stale: "预览已过期，请重新生成",
};

const reinstallFailureMessages: Record<ReinstallFailurePhase, string> = {
  planning: "无法生成重装计划，请重试",
  preflight: "提交前检查失败，请重新生成预览",
  lock: "当前游戏或配置档正在执行其他写入任务",
  backup: "创建安全快照失败，未提交新版本",
  commit: "提交新版本失败，后端已尝试恢复原状态",
  manifest: "写入安装记录失败，后端已进入受控恢复流程",
  post_commit: "新版本已提交，但收尾尚未完成，请在恢复中心完成收敛",
  rollback: "恢复原版本失败，请在恢复中心处理",
  complete: "重装任务收尾失败，请刷新状态后重试",
};

function commandErrorCode(error: unknown) {
  return typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string"
    ? error.code
    : null;
}

export function getReinstallPreviewErrorMessage(error: unknown) {
  switch (commandErrorCode(error)) {
    case "game_id_invalid":
      return "当前游戏不支持重装";
    case "profile_id_empty":
    case "mod_id_empty":
    case "candidate_revision_id_empty":
    case "layer_name_empty":
      return "重装请求已失效，请重新选择";
    default:
      return "无法生成重装预览，请稍后重试";
  }
}

export function getReinstallStartErrorMessage(error: unknown) {
  return commandErrorCode(error) === "plan_token_invalid"
    ? "重装预览已失效，请重新生成"
    : "无法启动重装任务，请重新生成预览后重试";
}

export function isReinstallTaskPhase(phase: string): phase is ReinstallTaskPhase {
  return Object.prototype.hasOwnProperty.call(reinstallTaskPhaseLabels, phase);
}

export function getReinstallTaskPhaseLabel(phase: ReinstallTaskPhase) {
  return reinstallTaskPhaseLabels[phase];
}

export function getReinstallBlockingReasonLabel(reason: ReinstallBlockingReason) {
  return reinstallBlockingReasonLabels[reason];
}

function parseReinstallFailurePhase(error: string | null): ReinstallFailurePhase | null {
  const prefix = "install_reinstall_failed:";
  if (!error?.startsWith(prefix)) {
    return null;
  }

  const phase = error.slice(prefix.length) as ReinstallFailurePhase;
  return Object.prototype.hasOwnProperty.call(reinstallFailureMessages, phase) ? phase : null;
}

export function nextReinstallTaskStateFromProgress(
  current: ReinstallTaskState,
  event: TaskProgressEventDto,
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
      message: failurePhase === null ? "重装失败，请刷新状态后重试" : reinstallFailureMessages[failurePhase],
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
  return installStatus === "installed" && preview.status === "ready" && task.status === "idle";
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
