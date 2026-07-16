import { invoke } from "@tauri-apps/api/core";
import type {
  AnalyzeImportedModReplacementInput,
  InitialRetargetInstallPreview,
  ListReplacementTargetsInput,
  PreviewInitialRetargetInstallInput,
  ReplacementAnalysis,
  ReplacementTarget,
  RetargetInstallTaskStarted,
  StartRetargetInstallTaskInput,
} from "./replacementTypes";

export function listReplacementTargets(
  input: ListReplacementTargetsInput,
): Promise<ReplacementTarget[]> {
  return invoke<ReplacementTarget[]>("list_replacement_targets", {
    request: {
      gameId: input.gameId,
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
