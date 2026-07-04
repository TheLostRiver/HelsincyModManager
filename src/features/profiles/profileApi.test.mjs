import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

const forbiddenDiscoveryFields = new RegExp(
  [
    "raw" + "Path",
    "full" + "Path",
    "steam" + "Id64",
    "account" + "Id",
    "x" + "ml",
    "profile" + "Url",
  ].join("|"),
  "i",
);

test("profile typed API invokes narrow profile commands without paths", () => {
  assert.equal(existsSync("src/features/profiles/profileApi.ts"), true);
  assert.equal(existsSync("src/features/profiles/profileTypes.ts"), true);

  const source = readSource("src/features/profiles/profileApi.ts");
  const typesSource = readSource("src/features/profiles/profileTypes.ts");

  assert.match(source, /invoke<Profile\[\]>\("list_profiles"/);
  assert.match(source, /invoke<Profile>\("get_active_profile"/);
  assert.match(source, /invoke<string>\("create_profile"/);
  assert.match(source, /invoke<void>\("update_profile"/);
  assert.match(source, /invoke<void>\("delete_profile"/);
  assert.match(source, /invoke<void>\("set_active_profile"/);
  assert.match(source, /name:\s*input\.name/);
  assert.match(source, /description:\s*input\.description/);
  assert.match(source, /profileId:\s*input\.profileId/);
  assert.doesNotMatch(source, /path|root|manifest|backup|sandbox|cache|target/i);

  assert.match(typesSource, /export type Profile = \{/);
  assert.match(typesSource, /id:\s*string/);
  assert.match(typesSource, /name:\s*string/);
  assert.match(typesSource, /description:\s*string\s*\|\s*null/);
  assert.match(typesSource, /isActive:\s*boolean/);
  assert.match(typesSource, /createdAt:\s*number/);
  assert.match(typesSource, /updatedAt:\s*number/);
});

test("profile hooks expose active profile refresh and mutation entry points", () => {
  assert.equal(existsSync("src/features/profiles/ActiveProfileProvider.tsx"), true);

  const source = readSource("src/features/profiles/ActiveProfileProvider.tsx");

  assert.match(source, /createContext<ActiveProfileContextValue\s*\|\s*null>/);
  assert.match(source, /getActiveProfile\(\)/);
  assert.match(source, /refreshActiveProfile/);
  assert.match(source, /setActiveProfile/);
  assert.match(source, /useActiveProfile/);
  assert.doesNotMatch(source, /DEFAULT_INSTALL_PROFILE_ID|profileId:\s*"default"/);
});

test("profile save settings API uses narrow settings commands", () => {
  const source = readSource("src/features/profiles/profileSaveSettingsApi.ts");
  const typesSource = readSource("src/features/profiles/profileSaveSettingsTypes.ts");

  assert.match(source, /getProfileSaveSettings\(input:\s*\{\s*gameId:\s*string;\s*profileId:\s*string;/);
  assert.match(source, /invoke<ProfileSaveSettingsDto>\("get_profile_save_settings",\s*input\)/);
  assert.match(source, /invoke<ProfileDirectoryValidationDto>\("validate_profile_save_directory"/);
  assert.match(source, /invoke<ProfileDirectoryValidationDto>\("validate_profile_backup_directory"/);
  assert.match(source, /invoke<ProfileSaveSettingsDto>\("set_profile_save_settings"/);
  assert.match(typesSource, /export type BackupCadence = "manual" \| "daily" \| "weekly"/);
  assert.match(typesSource, /pathLabel:\s*string\s*\|\s*null/);
  assert.doesNotMatch(typesSource, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
});

test("profile save backup API invokes task and history commands without filesystem details", () => {
  assert.equal(existsSync("src/features/profiles/profileSaveBackupApi.ts"), true);
  assert.equal(existsSync("src/features/profiles/profileSaveBackupTypes.ts"), true);

  const source = readSource("src/features/profiles/profileSaveBackupApi.ts");
  const typesSource = readSource("src/features/profiles/profileSaveBackupTypes.ts");

  assert.match(source, /startProfileSaveBackup\(input:\s*StartProfileSaveBackupInput\)/);
  assert.match(source, /invoke<TaskStartedDto>\("start_save_backup_task",\s*\{/);
  assert.match(source, /request:\s*\{/);
  assert.match(source, /gameId:\s*input\.gameId/);
  assert.match(source, /profileId:\s*input\.profileId/);
  assert.match(source, /note:\s*input\.note/);
  assert.match(source, /listProfileSaveBackups\(input:\s*ListProfileSaveBackupsInput\)/);
  assert.match(source, /invoke<SaveBackupSummaryDto\[\]>\("list_save_backups",\s*\{/);
  assert.match(source, /limit:\s*input\.limit/);
  assert.doesNotMatch(source, /path|root|manifest|backupRef|sandbox|cache|hash/i);

  assert.match(typesSource, /export type StartProfileSaveBackupInput = \{/);
  assert.match(typesSource, /export type ListProfileSaveBackupsInput = \{/);
  assert.match(typesSource, /export type TaskStartedDto = \{/);
  assert.match(typesSource, /kind:\s*"save_backup"/);
  assert.match(typesSource, /status:\s*"queued"/);
  assert.match(typesSource, /export type SaveBackupSummaryDto = \{/);
  assert.match(typesSource, /trigger:\s*"manual" \| "auto" \| "pre_install"/);
  assert.match(typesSource, /status:\s*"completed" \| "deleted_by_retention" \| "missing" \| "invalid"/);
  assert.match(typesSource, /fileName:\s*string/);
  assert.match(typesSource, /sourcePathLabel:\s*string\s*\|\s*null/);
  assert.doesNotMatch(typesSource, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache|hash/i);
});

test("profile save directory discovery API avoids raw paths and steam identifiers", () => {
  const source = readSource("src/features/profiles/profileSaveDirectoryDiscoveryApi.ts");
  const typesSource = readSource("src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts");

  assert.match(source, /discoverProfileSaveDirectories/);
  assert.match(source, /confirmProfileSaveDirectoryCandidate/);
  assert.match(source, /discoveryId:\s*input\.discoveryId/);
  assert.match(source, /candidateId:\s*input\.candidateId/);
  assert.doesNotMatch(source, forbiddenDiscoveryFields);

  assert.match(typesSource, /candidateId:\s*string/);
  assert.match(typesSource, /discoveryId:\s*string/);
  assert.match(typesSource, /accountName:\s*string\s*\|\s*null/);
  assert.match(typesSource, /avatarUrl:\s*string\s*\|\s*null/);
  assert.match(typesSource, /savedSettings\?:\s*ProfileDirectorySelectionDto\s*\|\s*null/);
  assert.doesNotMatch(typesSource, forbiddenDiscoveryFields);
});
