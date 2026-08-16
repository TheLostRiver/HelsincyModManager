import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const API_PATH = "src/features/settings/backgroundProtectionApi.ts";
const TYPES_PATH = "src/features/settings/backgroundProtectionTypes.ts";

function readProjectFile(path) {
  return readFileSync(path, "utf8");
}

test("background protection API uses only global narrow commands", () => {
  assert.equal(existsSync(API_PATH), true, "background protection API should exist");

  const source = readProjectFile(API_PATH);

  assert.match(source, /invoke<BackgroundProtectionControlDto>\(\s*"get_save_backup_background_control_status"/);
  assert.match(source, /invoke<BackgroundProtectionControlDto>\(\s*"enable_save_backup_background_protection"/);
  assert.match(source, /invoke<BackgroundProtectionControlDto>\(\s*"disable_save_backup_background_protection"/);
  assert.match(source, /peekBackgroundProtectionControlStatus/);
  assert.match(source, /cachedControlStatus/);
  assert.match(source, /pendingControlStatus/);
  assert.match(source, /options\?\.force/);
  assert.doesNotMatch(source, /taskName|taskXml|workerPath|workerId|PowerShell|sid|leaseOwner|savePath|backupPath/i);
});

test("background protection DTO exposes only stable global status fields", () => {
  assert.equal(existsSync(TYPES_PATH), true, "background protection types should exist");

  const source = readProjectFile(TYPES_PATH);

  assert.match(source, /export type BackgroundProtectionStatus\s*=/);
  for (const status of [
    "not_enabled",
    "starting",
    "protected",
    "registration_failed",
    "worker_unhealthy",
    "permission_required",
    "unsupported_platform",
  ]) {
    assert.match(source, new RegExp(`"${status}"`));
  }
  assert.match(source, /desiredEnabled:\s*boolean/);
  assert.match(source, /enabledAt:\s*number\s*\|\s*null/);
  assert.match(source, /lastHeartbeatAt:\s*number\s*\|\s*null/);
  assert.match(source, /lastErrorCode:\s*string\s*\|\s*null/);
  assert.doesNotMatch(source, /taskName|taskXml|workerPath|workerId|PowerShell|sid|leaseOwner|savePath|backupPath/i);
});
