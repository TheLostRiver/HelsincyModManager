import { invoke } from "@tauri-apps/api/core";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import type {
  AnalyzeImportedModReplacementInput,
  CancelRetargetInstallTaskInput,
  InitialRetargetInstallPreview,
  ListReplacementTargetsInput,
  PreviewInitialRetargetInstallInput,
  PreviewRetargetReinstallInput,
  ReplacementAnalysis,
  ReplacementTarget,
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
