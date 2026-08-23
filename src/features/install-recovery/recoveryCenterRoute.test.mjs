import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  isManualActionDisabled,
  resolveManualActionHandler,
} from "./recoveryCenterManualActions.ts";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("registers Recovery Center as a first-class enabled route and nav item", () => {
  const routeTypes = readSource("src/app/routing/routeTypes.ts");
  const routeRegistry = readSource("src/app/routing/routeRegistry.tsx");
  const navItems = readSource("src/app/shell/navigation/navItems.ts");
  const main = readSource("src/main.tsx");

  assert.match(routeTypes, /"recovery"/);
  assert.match(routeRegistry, /RecoveryCenterPage/);
  assert.match(routeRegistry, /id:\s*"recovery"/);
  assert.match(routeRegistry, /path:\s*"\/recovery"/);
  assert.match(navItems, /id:\s*"recovery"/);
  assert.match(navItems, /label:\s*"恢复中心"/);
  assert.match(navItems, /route:\s*"\/recovery"/);
  const recoveryNavLine = navItems
    .split("\n")
    .find((line) => line.includes('id: "recovery"'));
  assert.ok(recoveryNavLine);
  assert.equal(recoveryNavLine.includes("disabledReason"), false);
  assert.match(main, /RecoveryCenterPage\.css/);
});

test("Recovery Center scans with short ids and delegates rollback to controlled hook", () => {
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");
  const hook = readSource("src/features/install-recovery/useRecoveryCenterScan.ts");
  const rollbackHook = readSource("src/features/install-recovery/useRecoveryRollback.ts");

  assert.match(page, /useGameSetup/);
  assert.match(page, /useRecoveryCenterScan/);
  assert.match(page, /useRecoveryRollback/);
  assert.match(hook, /scanInstallRecovery/);
  assert.match(hook, /useActiveProfile/);
  assert.match(hook, /activeProfile\.status\s*!==\s*"ready"/);
  assert.match(hook, /gameId:\s*input\.gameId/);
  assert.match(hook, /profileId:\s*activeProfileId/);
  assert.match(hook, /modIds:\s*\[\]/);
  assert.match(rollbackHook, /previewRecoveryAction/);
  assert.match(rollbackHook, /startRecoveryActionTask/);
  assert.match(rollbackHook, /useActiveProfile/);
  assert.match(rollbackHook, /activeProfile\.status\s*!==\s*"ready"/);
  assert.match(rollbackHook, /profileId:\s*activeProfileId/);
  assert.match(rollbackHook, /actionKind:\s*"rollback_install"/);
  assert.match(rollbackHook, /notifyInstallRecoveryRefresh/);

  const forbidden = [
    "targetPath",
    "gameRoot",
    "backupRef",
    "backupRoot",
    "manifestPath",
    "manifestRoot",
    "sandbox",
    "cachePath",
    "targetHash",
    "restoreInstall",
    "rollbackInstall",
    "deleteInstall",
    "writeManifest",
  ];

  for (const token of forbidden) {
    assert.equal(page.includes(token), false, `${token} must stay out of Recovery Center UI`);
    assert.equal(hook.includes(token), false, `${token} must stay out of Recovery Center scan hook`);
    assert.equal(rollbackHook.includes(token), false, `${token} must stay out of Recovery Center rollback hook`);
  }
});

test("Recovery Center renders rich repair summary without direct install commands", () => {
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");

  assert.match(page, /RepairSummaryPanel/);
  assert.match(page, /summary\.blockingReason/);
  assert.match(page, /issue\.guidance/);
  assert.match(page, /aria-label=\{copy\.page\.repairAria\}/);
  // zh 值 pin 移到 copy 模块：防止字典改动悄悄改掉既定文案。
  const recoveryCopySource = readSource("src/features/install-recovery/recoveryCenterCopy.ts");
  assert.match(recoveryCopySource, /repairAria: "恢复处理摘要"/);

  const forbiddenCommands = ["startInstallTask", "startUninstallTask", "restoreInstall", "rollbackInstall", "deleteInstall"];
  for (const token of forbiddenCommands) {
    assert.equal(page.includes(token), false, `${token} must not be exposed from the Recovery Center`);
  }
});

test("Recovery Center renders manual handling decision panel with safe controlled recovery entry", () => {
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");

  assert.match(page, /ManualHandlingPanel/);
  assert.match(page, /manualDecision\.actions/);
  assert.match(page, /onRefresh/);
  assert.match(page, /onExportDiagnostics/);
  assert.match(page, /onScrollToModList/);
  assert.match(page, /isManualActionDisabled/);
  assert.match(page, /resolveManualActionHandler/);
  assert.match(page, /isRefreshing=\{scan\.state\.status === "loading"\}/);
  assert.match(page, /isExporting=\{diagnostics\.state\.status === "exporting"\}/);
  assert.match(page, /manual-decision/);

  const forbiddenCommands = [
    "restoreInstall",
    "rollbackInstall",
    "deleteInstall",
    "writeManifest",
    "startInstallTask",
    "startUninstallTask",
  ];

  for (const token of forbiddenCommands) {
    assert.equal(page.includes(token), false, `${token} must not be exposed from manual handling UI`);
  }
});

test("manual handling actions combine action state with live busy state", () => {
  const retryAction = {
    id: "retry_scan",
    label: "重新扫描",
    description: "重新读取后端只读恢复摘要。",
    state: "available",
  };
  const exportAction = {
    id: "export_diagnostics",
    label: "导出诊断",
    description: "生成已脱敏的支持诊断包。",
    state: "available",
  };
  const controlledRecoveryAction = {
    id: "controlled_recovery",
    label: "受控回滚",
    description: "在下方列表中使用逐 Mod 受控回滚。",
    state: "available",
  };
  const unavailableControlledRecoveryAction = {
    id: "controlled_recovery",
    label: "受控修复",
    description: "当前没有可执行受控回滚的 Mod。",
    state: "unavailable",
  };

  assert.equal(isManualActionDisabled(retryAction, { isRefreshing: false, isExporting: false }), false);
  assert.equal(isManualActionDisabled(retryAction, { isRefreshing: true, isExporting: false }), true);
  assert.equal(isManualActionDisabled(exportAction, { isRefreshing: false, isExporting: false }), false);
  assert.equal(isManualActionDisabled(exportAction, { isRefreshing: false, isExporting: true }), true);
  assert.equal(
    isManualActionDisabled(controlledRecoveryAction, { isRefreshing: false, isExporting: false }),
    false,
  );
  assert.equal(
    isManualActionDisabled(unavailableControlledRecoveryAction, { isRefreshing: false, isExporting: false }),
    true,
  );
});

test("manual handling action handlers only fire for available non-busy safe actions", () => {
  let refreshCount = 0;
  let exportCount = 0;
  let scrollCount = 0;
  const handlers = {
    onRefresh: () => {
      refreshCount += 1;
    },
    onExportDiagnostics: () => {
      exportCount += 1;
    },
    onScrollToModList: () => {
      scrollCount += 1;
    },
  };
  const retryAction = {
    id: "retry_scan",
    label: "重新扫描",
    description: "重新读取后端只读恢复摘要。",
    state: "available",
  };
  const exportAction = {
    id: "export_diagnostics",
    label: "导出诊断",
    description: "生成已脱敏的支持诊断包。",
    state: "available",
  };
  const controlledRecoveryAction = {
    id: "controlled_recovery",
    label: "受控回滚",
    description: "在下方列表中使用逐 Mod 受控回滚。",
    state: "available",
  };
  const unavailableControlledRecoveryAction = {
    id: "controlled_recovery",
    label: "受控修复",
    description: "当前没有可执行受控回滚的 Mod。",
    state: "unavailable",
  };

  resolveManualActionHandler(retryAction, { isRefreshing: false, isExporting: false }, handlers)?.();
  resolveManualActionHandler(exportAction, { isRefreshing: false, isExporting: false }, handlers)?.();
  resolveManualActionHandler(controlledRecoveryAction, { isRefreshing: false, isExporting: false }, handlers)?.();

  assert.equal(refreshCount, 1);
  assert.equal(exportCount, 1);
  assert.equal(scrollCount, 1);
  assert.equal(resolveManualActionHandler(retryAction, { isRefreshing: true, isExporting: false }, handlers), undefined);
  assert.equal(
    resolveManualActionHandler(exportAction, { isRefreshing: false, isExporting: true }, handlers),
    undefined,
  );
  assert.equal(
    resolveManualActionHandler(unavailableControlledRecoveryAction, { isRefreshing: false, isExporting: false }, handlers),
    undefined,
  );
});

test("Recovery Center rollback hook tracks task progress by task id and refreshes global health on completion", () => {
  const hook = readSource("src/features/install-recovery/useRecoveryRollback.ts");
  const healthHook = readSource("src/features/install-recovery/useInstallRecoveryHealth.ts");
  const refresh = readSource("src/features/install-recovery/installRecoveryRefresh.ts");

  assert.match(hook, /listen<\s*TaskProgressEventDto\s*>/);
  assert.match(hook, /event\.payload\.kind !== "install"/);
  assert.match(hook, /isRecoveryRollbackPhase\(phase\)/);
  assert.match(hook, /current\.status !== "running" \|\| current\.taskId !== event\.payload\.taskId/);
  assert.match(hook, /pendingEventsRef\.current\.set\(event\.payload\.taskId,\s*event\.payload\)/);
  assert.match(hook, /pendingEventsRef\.current\.get\(result\.taskId\)/);
  assert.match(hook, /install\.recovery\.completed/);
  assert.match(hook, /notifyInstallRecoveryRefresh\(\)/);

  assert.match(refresh, /INSTALL_RECOVERY_REFRESH_EVENT/);
  assert.match(refresh, /window\.dispatchEvent/);
  assert.match(refresh, /window\.addEventListener/);
  assert.match(healthHook, /subscribeInstallRecoveryRefresh/);
  assert.match(healthHook, /setRefreshToken\(\(current\) => current \+ 1\)/);
  assert.match(healthHook, /\[activeProfile\.status,\s*activeProfileId,\s*input\.enabled,\s*input\.gameId,\s*refreshToken\]/);
});

test("Recovery Center exposes support diagnostics export without path or raw log fields", () => {
  const api = readSource("src/features/install-recovery/recoveryDiagnosticsApi.ts");
  const types = readSource("src/features/install-recovery/recoveryDiagnosticsTypes.ts");
  const hook = readSource("src/features/install-recovery/useRecoveryDiagnosticsExport.ts");
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");

  assert.match(api, /export function exportSupportDiagnostics/);
  assert.match(api, /invoke<SupportDiagnosticsExport>\("export_support_diagnostics"\)/);
  assert.doesNotMatch(api, /request:\s*\{|outputPath|logPath|path:/i);

  assert.match(types, /export type SupportDiagnosticsExport/);
  assert.match(types, /exportId:\s*string/);
  assert.match(types, /fileName:\s*string/);
  assert.match(types, /sizeBytes:\s*number/);
  assert.match(types, /appLogLineCount:\s*number/);
  assert.match(types, /debugLogLineCount:\s*number/);
  assert.match(types, /taskLogLineCount:\s*number/);
  assert.match(types, /auditEventCount:\s*number/);
  for (const field of [
    "debugLogStatus",
    "taskLogStatus",
    "auditLogStatus",
    "logStorageStatus",
    "debugLogEventRejectedCount",
    "debugLogWriteFailureCount",
    "debugLogRetentionFailureCount",
    "taskLogWriteFailureCount",
    "taskLogRetentionFailureCount",
    "auditWriteFailureCount",
    "auditWriteFailureAfterCommitCount",
    "auditLogRetentionFailureCount",
    "logStorageFailureCount",
    "logStorageUnsatisfiedCount",
    "logStorageSettingsFailureCount",
  ]) {
    assert.match(types, new RegExp(`${field}:\\s*(?:string|number)`));
  }

  assert.match(hook, /exportSupportDiagnostics/);
  assert.match(hook, /status:\s*"confirming"/);
  assert.match(hook, /status:\s*"exporting"/);
  assert.match(hook, /useFeedback/);
  assert.match(hook, /eventKey:\s*`recovery\.diagnostics\.exported\.\$\{result\.exportId\}`/);
  assert.match(hook, /eventKey:\s*"recovery\.diagnostics\.export\.failed"/);
  assert.match(hook, /requestExport/);
  assert.match(hook, /confirmExport/);
  assert.match(hook, /cancelExport/);

  assert.match(page, /useRecoveryDiagnosticsExport/);
  assert.match(page, /DiagnosticExportPanel/);
  assert.match(page, /diagnostics\.state/);
  assert.match(page, /diagCopy\.confirmTitle/);
  assert.match(
    readSource("src/features/install-recovery/recoveryCenterCopy.ts"),
    /confirmTitle: "确认导出诊断包"/,
  );
  assert.match(page, /onConfirm/);
  assert.match(page, /onCancel/);
  assert.doesNotMatch(page, /result\.fileName|result\.sizeBytes|result\.appLogLineCount|result\.taskLogLineCount|result\.auditEventCount/);

  const forbidden = [
    "outputPath",
    "logPath",
    "diagnosticsPath",
    "appLogLines",
    "taskLogLines",
    "events",
    "targetPath",
    "gameRoot",
    "backupRef",
    "backupRoot",
    "manifestPath",
    "manifestRoot",
    "sandbox",
    "cachePath",
    "raw_path",
  ];

  for (const token of forbidden) {
    assert.equal(api.includes(token), false, `${token} must stay out of diagnostics API`);
    assert.equal(types.includes(token), false, `${token} must stay out of diagnostics types`);
    assert.equal(hook.includes(token), false, `${token} must stay out of diagnostics hook`);
    assert.equal(page.includes(token), false, `${token} must stay out of Recovery Center diagnostics UI`);
  }
});
