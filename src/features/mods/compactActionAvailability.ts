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

export const MOD_LIBRARY_QUERY_BUSY_MESSAGE = "Mod 列表正在更新，请稍候";

export function getCompactActionDisabledReason({
  actionId,
  selectedCount,
  profileReady,
  installTaskActive,
  libraryQueryBusy,
  canInstallSelection,
  canReinstallSelection,
  canUninstallSelection,
}: CompactActionAvailabilityInput): string | undefined {
  if (
    libraryQueryBusy
    && (actionId === "select-all"
      || actionId === "invert"
      || lifecycleActions.has(actionId as CompactLifecycleActionId))
  ) {
    return MOD_LIBRARY_QUERY_BUSY_MESSAGE;
  }
  if (!lifecycleActions.has(actionId as CompactLifecycleActionId)) {
    return undefined;
  }
  if (selectedCount === 0) {
    return "请先选择一个 MOD";
  }
  if (selectedCount > 1) {
    // T13-07: multi-selection drives the batch mod lifecycle flow. Per-operation feasibility
    // is decided below by the batch canXxxSelection facts; items without an applicable state
    // are excluded inside the batch preview with stable reasons.
  }
  if (installTaskActive) {
    return "请等待当前安装任务完成";
  }
  if (!profileReady) {
    const actionLabel: Record<CompactLifecycleActionId, string> = {
      "preview-plan": "预览安装计划",
      install: "安装",
      reinstall: "重装",
      uninstall: "卸载",
    };
    return `选择配置档后可${actionLabel[actionId as CompactLifecycleActionId]}`;
  }

  switch (actionId) {
    case "preview-plan":
      return canInstallSelection ? undefined : "仅未安装且状态安全的 MOD 可预览安装计划";
    case "install":
      return canInstallSelection ? undefined : "仅未安装且状态安全的 MOD 可安装";
    case "reinstall":
      return canReinstallSelection ? undefined : "仅已安装且状态安全的 MOD 可重装";
    case "uninstall":
      return canUninstallSelection ? undefined : "仅已安装且状态安全的 MOD 可卸载";
    default:
      return undefined;
  }
}
