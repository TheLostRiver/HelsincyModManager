import { invoke } from "@tauri-apps/api/core";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import type {
  AnalyzeImportedModReplacementInput,
  CancelRetargetInstallTaskInput,
  InitialRetargetInstallPreview,
  ListReplacementTargetOccupancyInput,
  ListReplacementTargetsInput,
  PreviewInitialRetargetInstallInput,
  PreviewRetargetReinstallInput,
  ReplacementAnalysis,
  ReplacementTarget,
  OccupiedReplacementTarget,
  RetargetInstallTaskStarted,
  StartRetargetInstallTaskInput,
  StartRetargetReinstallTaskInput,
} from "./replacementTypes";

export function listReplacementTargets(
  input: ListReplacementTargetsInput,
): Promise<ReplacementTarget[]> {
  return invoke<ReplacementTarget[]>("list_replacement_targets", {
    request: {
      gameId: input.gameId,
      modId: input.modId,
      query: input.query,
    },
  });
}

export function analyzeImportedModReplacement(
  input: AnalyzeImportedModReplacementInput,
): Promise<ReplacementAnalysis> {
  return invoke<ReplacementAnalysis>("analyze_imported_mod_replacement", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modId: input.modId,
    },
  });
}

/**
 * 跨 Mod 同目标占用查询，仅用于 UI 提示。
 *
 * 后端在清单不可信或读取失败时返回空列表（fail-open）；硬门禁不在这个
 * command 上，而在预览、任务期计划构建与 commit 三层。调用失败不该打断
 * 面板加载，因此这里不额外包装错误。
 */
export function listReplacementTargetOccupancy(
  input: ListReplacementTargetOccupancyInput,
): Promise<OccupiedReplacementTarget[]> {
  return invoke<OccupiedReplacementTarget[]>("list_replacement_target_occupancy", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modId: input.modId,
    },
  });
}

export function previewInitialRetargetInstall(
  input: PreviewInitialRetargetInstallInput,
): Promise<InitialRetargetInstallPreview> {
  return invoke<InitialRetargetInstallPreview>("preview_initial_retarget_install", {
    request: initialRetargetRequest(input),
  });
}

export function startRetargetInstallTask(
  input: StartRetargetInstallTaskInput,
): Promise<RetargetInstallTaskStarted> {
  return invoke<RetargetInstallTaskStarted>("start_retarget_install_task", {
    request: initialRetargetRequest(input),
  });
}

export function previewRetargetReinstall(
  input: PreviewRetargetReinstallInput,
): Promise<ReinstallPlanPreview> {
  return invoke<ReinstallPlanPreview>("preview_retarget_reinstall", {
    request: retargetReinstallRequest(input),
  });
}

export function startRetargetReinstallTask(
  input: StartRetargetReinstallTaskInput,
): Promise<RetargetInstallTaskStarted> {
  return invoke<RetargetInstallTaskStarted>("start_retarget_reinstall_task", {
    request: {
      ...retargetReinstallRequest(input),
      planToken: input.planToken,
    },
  });
}

export function cancelRetargetInstallTask(
  input: CancelRetargetInstallTaskInput,
): Promise<RetargetInstallTaskStarted> {
  return invoke<RetargetInstallTaskStarted>("cancel_task", {
    taskId: input.taskId,
  });
}

function initialRetargetRequest(input: PreviewInitialRetargetInstallInput) {
  return {
    gameId: input.gameId,
    profileId: input.profileId,
    modId: input.modId,
    targetId: input.targetId,
    layerName: input.layerName,
    layerPriority: input.layerPriority,
  };
}

function retargetReinstallRequest(input: PreviewRetargetReinstallInput) {
  return initialRetargetRequest(input);
}
