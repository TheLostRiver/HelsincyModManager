import type { GameId } from "../game-setup/gameSetupTypes";
import type { InitialRetargetInstallPreview, ReplacementSource, ReplacementTarget, ReplacementWarning } from "./replacementTypes";

export type EquipmentSourceConfiguration = {
  source: ReplacementSource;
  originalTargetId: string | null;
  targets: ReplacementTarget[];
};

export type EquipmentRetargetConfiguration = {
  gameId: string;
  modId: string;
  sources: EquipmentSourceConfiguration[];
  installedTargets: Record<string, string> | null;
  warnings: ReplacementWarning[];
};

export type EquipmentSlotIntent =
  | { action: "keep"; sourceId: string }
  | { action: "retarget"; sourceId: string; targetId: string };

export type EquipmentRetargetSelection = {
  gameId: GameId;
  profileId: string;
  modId: string;
  slots: EquipmentSlotIntent[];
  layerName: string;
  layerPriority: number;
};

export type EquipmentReapplyInput = Pick<EquipmentRetargetSelection, "gameId" | "profileId" | "modId">;

export type EquipmentRetargetPreviewError = { code: string; message: string; sourceId?: string };

export function equipmentErrorSourceId(error: unknown): string | null {
  return typeof error === "object" && error !== null && "sourceId" in error && typeof error.sourceId === "string"
    ? error.sourceId : null;
}

export type EquipmentRetargetInstallPreview = Omit<InitialRetargetInstallPreview, "target" | "actions"> & {
  targets: ReplacementTarget[];
};

export type EquipmentTargetChoice = { targetId: string; alias: string | null } | null;
export type EquipmentTargetChoices = Record<string, EquipmentTargetChoice>;

export function initialEquipmentChoices(configuration: EquipmentRetargetConfiguration): EquipmentTargetChoices {
  return Object.fromEntries(configuration.sources.map(({ source, originalTargetId }) => {
    const installed = configuration.installedTargets?.[source.id];
    return [source.id, installed && installed !== originalTargetId ? { targetId: installed, alias: null } : null];
  }));
}

export function equipmentSlotIntents(configuration: EquipmentRetargetConfiguration, choices: EquipmentTargetChoices): EquipmentSlotIntent[] {
  return configuration.sources.map(({ source }) => {
    const choice = choices[source.id];
    return choice ? { action: "retarget", sourceId: source.id, targetId: choice.targetId } : { action: "keep", sourceId: source.id };
  });
}
