import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("dashboard wires app-level install recovery health scan into setup rail", () => {
  assert.equal(existsSync("src/features/dashboard/useInstallRecoveryHealth.ts"), true);
  assert.equal(existsSync("src/features/dashboard/InstallRecoveryHealthPanel.tsx"), true);

  const dashboardSource = readSource("src/features/dashboard/DashboardPage.tsx");
  const setupPanelSource = readSource("src/features/dashboard/SetupStatusPanel.tsx");

  assert.match(dashboardSource, /useInstallRecoveryHealth/);
  assert.match(dashboardSource, /enabled:\s*gameSetup\.status\.kind\s*===\s*"configured"/);
  assert.match(dashboardSource, /recoveryHealth=\{recoveryHealth\}/);
  assert.match(setupPanelSource, /InstallRecoveryHealthPanel/);
  assert.match(setupPanelSource, /recoveryHealth/);
});

test("dashboard recovery health hook uses profile-wide readonly scan without paths", () => {
  const hookSource = readSource("src/features/install-recovery/useInstallRecoveryHealth.ts");

  assert.match(hookSource, /scanInstallRecovery/);
  assert.match(hookSource, /subscribeInstallRecoveryRefresh/);
  assert.match(hookSource, /gameId:\s*input\.gameId/);
  assert.match(hookSource, /profileId:\s*DEFAULT_INSTALL_PROFILE_ID/);
  assert.match(hookSource, /modIds:\s*\[\]/);
  assert.match(hookSource, /\[input\.enabled,\s*input\.gameId,\s*refreshToken\]/);
  assert.doesNotMatch(
    hookSource,
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|manifestRoot|backupRoot|backupRef/i,
  );
});

test("dashboard recovery health UI displays only aggregate safe fields", () => {
  const panelSource = readSource("src/features/dashboard/InstallRecoveryHealthPanel.tsx");
  const healthSource = readSource("src/features/dashboard/installRecoveryHealth.ts");

  assert.match(panelSource, /scannedModCount/);
  assert.match(panelSource, /attentionModCount/);
  assert.match(panelSource, /unknownModCount/);
  assert.match(panelSource, /issueCount/);
  assert.match(healthSource, /deriveInstallRecoveryHealth/);
  assert.doesNotMatch(
    `${panelSource}\n${healthSource}`,
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|manifestRoot|backupRoot|backupRef|gameRoot|targetHash/i,
  );
});
