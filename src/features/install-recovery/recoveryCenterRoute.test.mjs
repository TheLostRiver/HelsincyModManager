import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

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

test("Recovery Center page only performs read-only recovery scan with short ids", () => {
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");
  const hook = readSource("src/features/install-recovery/useRecoveryCenterScan.ts");

  assert.match(page, /useGameSetup/);
  assert.match(page, /useRecoveryCenterScan/);
  assert.match(hook, /scanInstallRecovery/);
  assert.match(hook, /gameId:\s*input\.gameId/);
  assert.match(hook, /profileId:\s*DEFAULT_INSTALL_PROFILE_ID/);
  assert.match(hook, /modIds:\s*\[\]/);

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
  }
});

test("Recovery Center renders rich repair summary without action commands", () => {
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");

  assert.match(page, /RepairSummaryPanel/);
  assert.match(page, /summary\.blockingReason/);
  assert.match(page, /issue\.guidance/);
  assert.match(page, /aria-label="恢复处理摘要"/);

  const forbiddenCommands = ["startInstallTask", "startUninstallTask", "restoreInstall", "rollbackInstall", "deleteInstall"];
  for (const token of forbiddenCommands) {
    assert.equal(page.includes(token), false, `${token} must not be exposed from the Recovery Center`);
  }
});

test("Recovery Center renders manual handling decision panel with safe actions only", () => {
  const page = readSource("src/features/install-recovery/RecoveryCenterPage.tsx");

  assert.match(page, /ManualHandlingPanel/);
  assert.match(page, /manualDecision\.actions/);
  assert.match(page, /onRefresh/);
  assert.match(page, /onExportDiagnostics/);
  assert.match(page, /action\.state === "available"/);
  assert.match(page, /disabled=\{action\.state !== "available"\}/);
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
  assert.match(types, /taskLogLineCount:\s*number/);
  assert.match(types, /auditEventCount:\s*number/);

  assert.match(hook, /exportSupportDiagnostics/);
  assert.match(hook, /status:\s*"confirming"/);
  assert.match(hook, /status:\s*"exporting"/);
  assert.match(hook, /status:\s*"exported"/);
  assert.match(hook, /status:\s*"failed"/);
  assert.match(hook, /requestExport/);
  assert.match(hook, /confirmExport/);
  assert.match(hook, /cancelExport/);

  assert.match(page, /useRecoveryDiagnosticsExport/);
  assert.match(page, /DiagnosticExportPanel/);
  assert.match(page, /diagnostics\.state/);
  assert.match(page, /确认导出诊断包/);
  assert.match(page, /onConfirm/);
  assert.match(page, /onCancel/);
  assert.match(page, /result\.fileName/);
  assert.match(page, /result\.sizeBytes/);
  assert.match(page, /result\.appLogLineCount/);
  assert.match(page, /result\.taskLogLineCount/);
  assert.match(page, /result\.auditEventCount/);

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
