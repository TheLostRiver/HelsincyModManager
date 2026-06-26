import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

const baseHealth = {
  status: "attention",
  scannedModCount: 3,
  completedModCount: 1,
  attentionModCount: 1,
  unknownModCount: 1,
  managedFileCount: 4,
  backupCount: 2,
  issueCount: 2,
  issues: [
    { issue: "target_changed", count: 1 },
    { issue: "backup_missing", count: 1 },
  ],
};

test("derives global recovery attention alert from unsafe profile health without paths", async () => {
  const { deriveInstallRecoveryGlobalAlert } = await import("./installRecoveryGlobalAlert.ts");

  const alert = deriveInstallRecoveryGlobalAlert({
    status: "ready",
    health: baseHealth,
  });

  assert.equal(alert.status, "attention");
  assert.equal(alert.title, "托管安装需要处理");
  assert.match(alert.description, /1 个需处理/);
  assert.match(alert.description, /1 个状态未知/);
  assert.equal(alert.actionLabel, "打开恢复中心");
  assert.equal("targetPath" in alert, false);
  assert.equal("backupRef" in alert, false);
  assert.equal("manifestPath" in alert, false);
});

test("hides global recovery alert for healthy, empty, idle and loading states", async () => {
  const { deriveInstallRecoveryGlobalAlert } = await import("./installRecoveryGlobalAlert.ts");

  assert.equal(deriveInstallRecoveryGlobalAlert({ status: "idle" }), null);
  assert.equal(deriveInstallRecoveryGlobalAlert({ status: "loading" }), null);
  assert.equal(
    deriveInstallRecoveryGlobalAlert({
      status: "ready",
      health: { ...baseHealth, status: "healthy", attentionModCount: 0, unknownModCount: 0, issueCount: 0, issues: [] },
    }),
    null,
  );
  assert.equal(
    deriveInstallRecoveryGlobalAlert({
      status: "ready",
      health: { ...baseHealth, status: "empty", scannedModCount: 0, attentionModCount: 0, unknownModCount: 0 },
    }),
    null,
  );
});

test("derives unavailable global recovery alert without raw error details", async () => {
  const { deriveInstallRecoveryGlobalAlert } = await import("./installRecoveryGlobalAlert.ts");

  const alert = deriveInstallRecoveryGlobalAlert({ status: "unavailable" });

  assert.equal(alert.status, "unknown");
  assert.equal(alert.title, "恢复摘要暂时不可用");
  assert.equal(alert.actionLabel, "打开恢复中心");
  assert.doesNotMatch(alert.description, /raw|path|manifest|backup/i);
});

test("describes unknown-only global recovery attention without zero-count noise", async () => {
  const { deriveInstallRecoveryGlobalAlert } = await import("./installRecoveryGlobalAlert.ts");

  const alert = deriveInstallRecoveryGlobalAlert({
    status: "ready",
    health: {
      ...baseHealth,
      attentionModCount: 0,
      unknownModCount: 2,
      issueCount: 0,
      issues: [],
    },
  });

  assert.equal(alert.status, "attention");
  assert.match(alert.description, /2 个状态未知/);
  assert.doesNotMatch(alert.description, /0 个需处理/);
});

test("app frame wires readonly global recovery alert to recovery center navigation", () => {
  assert.equal(existsSync("src/features/install-recovery/InstallRecoveryGlobalAlertPanel.tsx"), true);

  const frameSource = readSource("src/app/frame/AppFrame.tsx");
  const alertSource = readSource("src/features/install-recovery/InstallRecoveryGlobalAlertPanel.tsx");

  assert.match(frameSource, /InstallRecoveryGlobalAlert/);
  assert.match(alertSource, /useInstallRecoveryHealth/);
  assert.match(alertSource, /enabled:\s*gameSetup\.status\.kind\s*===\s*"configured"/);
  assert.match(alertSource, /navigate\("\/recovery"\)/);
  assert.doesNotMatch(
    alertSource,
    /targetPath|allowedTargetRoots|archivePath|sandbox|cache|rawPath|manifestPath|manifestRoot|backupRoot|backupRef|gameRoot|targetHash/i,
  );
});
