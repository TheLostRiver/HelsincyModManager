import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

test("profile view model maps save settings statuses without exposing raw paths", () => {
  const source = readFileSync("src/features/profiles/profileViewModel.ts", "utf8");

  assert.match(source, /formatDirectoryStatus/);
  assert.match(source, /formatBackupSchedule/);
  assert.match(source, /pathLabel/);
  assert.doesNotMatch(source, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
});
