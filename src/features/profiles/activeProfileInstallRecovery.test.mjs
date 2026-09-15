import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("mod library install and recovery status calls use installation scope id", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /useModInstallation/);
  assert.match(source, /installationScopeId/);
  assert.match(source, /profileId:\s*installationScopeId/);
  assert.doesNotMatch(source, /DEFAULT_INSTALL_PROFILE_ID/);
  assert.doesNotMatch(source, /profileId:\s*"default"/);
});

test("recovery center scan and rollback hooks use installation scope id", () => {
  const scanSource = readSource("src/features/install-recovery/useRecoveryCenterScan.ts");
  const rollbackSource = readSource("src/features/install-recovery/useRecoveryRollback.ts");
  const healthSource = readSource("src/features/install-recovery/useInstallRecoveryHealth.ts");

  for (const source of [scanSource, rollbackSource, healthSource]) {
    assert.match(source, /useModInstallation/);
    assert.match(source, /installationScopeId/);
    assert.match(source, /profileId:\s*installationScopeId/);
    assert.doesNotMatch(source, /DEFAULT_INSTALL_PROFILE_ID/);
    assert.doesNotMatch(source, /profileId:\s*"default"/);
  }
});

test("installation-dependent install and recovery hooks stay idle until game installation is ready", () => {
  const modLibrarySource = readSource("src/features/mods/ModLibraryPage.tsx");
  const scanSource = readSource("src/features/install-recovery/useRecoveryCenterScan.ts");
  const healthSource = readSource("src/features/install-recovery/useInstallRecoveryHealth.ts");

  assert.match(modLibrarySource, /installationScope\.status\s*!==\s*"ready"/);
  assert.match(scanSource, /installationScope\.status\s*!==\s*"ready"/);
  assert.match(healthSource, /installationScope\.status\s*!==\s*"ready"/);
});
