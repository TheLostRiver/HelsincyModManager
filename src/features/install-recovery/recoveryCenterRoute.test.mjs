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
