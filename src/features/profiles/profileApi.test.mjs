import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

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

  assert.match(source, /invoke<ProfileSaveSettingsDto>\("get_profile_save_settings"/);
  assert.match(source, /invoke<ProfileDirectoryValidationDto>\("validate_profile_save_directory"/);
  assert.match(source, /invoke<ProfileDirectoryValidationDto>\("validate_profile_backup_directory"/);
  assert.match(source, /invoke<ProfileSaveSettingsDto>\("set_profile_save_settings"/);
  assert.match(typesSource, /export type BackupCadence = "manual" \| "daily" \| "weekly"/);
  assert.match(typesSource, /pathLabel:\s*string\s*\|\s*null/);
  assert.doesNotMatch(typesSource, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
});
