import type { RecoveryCenterManualAction } from "./recoveryCenterViewModel";

export type RecoveryCenterManualBusyState = {
  isRefreshing: boolean;
  isExporting: boolean;
};

export type RecoveryCenterManualActionHandlers = {
  onRefresh: () => void;
  onExportDiagnostics: () => void;
  onScrollToModList: () => void;
};

export function isManualActionDisabled(
  action: RecoveryCenterManualAction,
  busyState: RecoveryCenterManualBusyState,
) {
  return action.state !== "available" || !isSupportedManualAction(action) || isManualActionBusy(action, busyState);
}

export function resolveManualActionHandler(
  action: RecoveryCenterManualAction,
  busyState: RecoveryCenterManualBusyState,
  handlers: RecoveryCenterManualActionHandlers,
) {
  if (isManualActionDisabled(action, busyState)) {
    return undefined;
  }

  if (action.id === "retry_scan") {
    return handlers.onRefresh;
  }

  if (action.id === "export_diagnostics") {
    return handlers.onExportDiagnostics;
  }

  if (action.id === "controlled_recovery") {
    return handlers.onScrollToModList;
  }

  return undefined;
}

function isSupportedManualAction(action: RecoveryCenterManualAction) {
  return action.id === "retry_scan" || action.id === "export_diagnostics" || action.id === "controlled_recovery";
}

function isManualActionBusy(action: RecoveryCenterManualAction, busyState: RecoveryCenterManualBusyState) {
  return (
    (action.id === "retry_scan" && busyState.isRefreshing) ||
    (action.id === "export_diagnostics" && busyState.isExporting)
  );
}
