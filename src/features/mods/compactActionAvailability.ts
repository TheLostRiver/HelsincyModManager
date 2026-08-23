import type { ModLibraryCopy } from "./modLibraryCopy";

export type CompactLifecycleActionId = "preview-plan" | "install" | "reinstall" | "uninstall";

type CompactActionAvailabilityInput = {
  actionId: string;
  selectedCount: number;
  profileReady: boolean;
  installTaskActive: boolean;
  libraryQueryBusy: boolean;
  canInstallSelection: boolean;
  canReinstallSelection: boolean;
  canUninstallSelection: boolean;
};

const lifecycleActions = new Set<CompactLifecycleActionId>([
  "preview-plan",
  "install",
  "reinstall",
  "uninstall",
]);

export function getCompactActionDisabledReason({
  actionId,
  selectedCount,
  profileReady,
  installTaskActive,
  libraryQueryBusy,
  canInstallSelection,
  canReinstallSelection,
  canUninstallSelection,
}: CompactActionAvailabilityInput, compact: ModLibraryCopy["compact"]): string | undefined {
  if (
    libraryQueryBusy
    && (actionId === "select-all"
      || actionId === "invert"
      || lifecycleActions.has(actionId as CompactLifecycleActionId))
  ) {
    return compact.queryBusy;
  }
  if (!lifecycleActions.has(actionId as CompactLifecycleActionId)) {
    return undefined;
  }
  if (selectedCount === 0) {
    return compact.selectOneFirst;
  }
  if (selectedCount > 1) {
    // T13-07: multi-selection drives the batch mod lifecycle flow. Per-operation feasibility
    // is decided below by the batch canXxxSelection facts; items without an applicable state
    // are excluded inside the batch preview with stable reasons.
  }
  if (installTaskActive) {
    return compact.waitInstallTask;
  }
  if (!profileReady) {
    const actionLabel: Record<CompactLifecycleActionId, string> = {
      "preview-plan": compact.actionLabels.previewPlan,
      install: compact.actionLabels.install,
      reinstall: compact.actionLabels.reinstall,
      uninstall: compact.actionLabels.uninstall,
    };
    return compact.selectProfileFor(actionLabel[actionId as CompactLifecycleActionId]);
  }

  switch (actionId) {
    case "preview-plan":
      return canInstallSelection ? undefined : compact.previewNeedsInstallable;
    case "install":
      return canInstallSelection ? undefined : compact.installNeedsInstallable;
    case "reinstall":
      return canReinstallSelection ? undefined : compact.reinstallNeedsInstalled;
    case "uninstall":
      return canUninstallSelection ? undefined : compact.uninstallNeedsInstalled;
    default:
      return undefined;
  }
}
