import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "./modImportTypes";
import type {
  PreviewReinstallPlanInput,
  ReinstallPlanPreview,
  StartReinstallTaskInput,
} from "./modReinstallTypes";

export function previewReinstallPlan(
  input: PreviewReinstallPlanInput,
): Promise<ReinstallPlanPreview> {
  return invoke<ReinstallPlanPreview>("preview_reinstall_plan", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modId: input.modId,
      candidateRevisionId: input.candidateRevisionId,
      layer: input.layer,
    },
  });
}

export function startReinstallTask(input: StartReinstallTaskInput): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("start_reinstall_task", {
    request: {
      gameId: input.gameId,
      profileId: input.profileId,
      modId: input.modId,
      candidateRevisionId: input.candidateRevisionId,
      layer: input.layer,
      planToken: input.planToken,
    },
  });
}
